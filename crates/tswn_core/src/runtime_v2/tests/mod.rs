use super::*;

mod entity_identity_tests;
mod plain_action_scheduler_tests;
mod plain_assassinate_skill_tests;
mod plain_at_boost_precision_tests;
mod plain_attack_skill_tests;
mod plain_charm_targeting_tests;
mod plain_clone_skill_tests;
mod plain_counter_skill_tests;
mod plain_damage_short_circuit_tests;
mod plain_disperse_skill_tests;
mod plain_haste_scheduler_tests;
mod plain_haste_targeting_tests;
mod plain_heal_skill_tests;
mod plain_ice_scheduler_tests;
mod plain_ice_skill_tests;
mod plain_kill_hook_tests;
mod plain_linked_minion_cleanup_tests;
mod plain_poison_state_tests;
mod plain_protect_skill_tests;
mod plain_raw_import_tests;
mod plain_reflect_skill_tests;
mod plain_revive_lifecycle_tests;
mod plain_status_skill_tests;
mod plain_summon_share_damage_tests;
mod plain_summon_skill_tests;
mod plain_terminal_round_tests;
mod plain_zombie_skill_tests;
mod prepared_init_tests;
mod summon_explode_scheduler_tests;
mod summon_explode_tests;

fn normalized_rng_checkpoint(i: u32, j: u32) -> crate::runtime_v2::oracle::NormalizedRngCheckpoint {
    crate::runtime_v2::oracle::NormalizedRngCheckpoint {
        i,
        j,
        #[cfg(not(feature = "no_debug"))]
        byte_count: 0,
    }
}

fn assert_rng_state_eq(actual: &RC4, expected: &RC4) {
    assert_eq!(actual.i, expected.i);
    assert_eq!(actual.j, expected.j);
    assert_eq!(actual.main_val, expected.main_val);
}

fn plain_large_expected_round(
    round: u64,
    winner_team: Option<usize>,
    score: u64,
    rng_i: u32,
    rng_j: u32,
    hp: [i32; 2],
    alive: [bool; 2],
    action: [usize; 2],
) -> NormalizedOutcome {
    let team_alive = vec![
        alive[0].then_some(0).into_iter().collect::<Vec<_>>(),
        alive[1].then_some(1).into_iter().collect::<Vec<_>>(),
    ];
    let flat_alive = alive
        .iter()
        .enumerate()
        .filter_map(|(idx, is_alive)| is_alive.then_some(idx))
        .collect::<Vec<_>>();
    let round_order = flat_alive.clone();
    let alive_group_count = team_alive.iter().filter(|team| !team.is_empty()).count();
    NormalizedOutcome {
        winner_team,
        round,
        total_score: score,
        rng: normalized_rng_checkpoint(rng_i, rng_j),
        entity_ids: vec![1, 2],
        teams: vec![0, 1],
        hp: hp.to_vec(),
        magic_point: vec![28, 29],
        defense: vec![58, 52],
        resistance: vec![49, 57],
        alive: alive.to_vec(),
        round_order,
        flat_alive,
        team_alive,
        alive_group_count,
        actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
            round,
            actor: action[0],
            target: action[1],
            amount: score as i32,
        }],
        frames: vec![NormalizedUpdateFrame {
            message: "[0]攻击[1]".to_owned(),
            caster: action[0],
            target: action[1],
            targets: Vec::new(),
            param: None,
            score: score as u32,
            delay0: crate::engine::update::DEFAULT_DELAY0_MS,
            delay1: crate::engine::update::DEFAULT_DELAY1_MS,
            update_type: crate::engine::update::UpdateType::None,
        }],
    }
}

#[test]
fn minimal_1v1_template_builds_runtime() {
    let runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));

    assert_eq!(runtime.entities.len(), 2);
    assert_eq!(runtime.world.winner_team(), None);
    assert!(runtime.effects.is_empty());
    assert!(runtime.slots.is_empty());
    assert_eq!(runtime.validate_ready(), Ok(()));
}

#[test]
fn runtime_ready_validation_reports_entity_and_template_skill_sources() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill("custom", "missing", "custom.missing", TargetPolicy::Enemy, SkillPriority(0))
        .expect("skill should register");
    let template_slot = builder
        .reserve_template_slot("custom", "spawn", "custom.spawn")
        .expect("template slot should reserve");
    let registry = builder.build();
    let mut template =
        PreparedCombatTemplate::with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill])], registry);
    template
        .slots
        .set(
            template_slot,
            SlotValue::PlayerTemplate(Box::new(PlayerTemplate::new(2, "spawn", 0, 5, 1).with_skills([skill]))),
        )
        .expect("template slot should accept player template");
    let mut runtime = CombatRuntime::from_template(template);

    let error = runtime.validate_ready().expect_err("missing handler should reject runtime");
    assert_eq!(
        error,
        RuntimeV2ReadyError {
            missing_skill_handlers: vec![RuntimeV2MissingSkillHandler {
                skill_id: skill,
                export_name: Some("custom.missing".to_owned()),
                sources: vec![
                    RuntimeV2SkillSource::Entity(EntityIdx(0)),
                    RuntimeV2SkillSource::TemplateSlot(template_slot),
                ],
            }],
        }
    );
    assert_eq!(
        error.to_string(),
        "runtime v2 missing skill handlers: custom.missing (id 0) used by entity 0, template slot 0"
    );

    runtime.set_skill_handler(skill, skill_noop);
    assert_eq!(runtime.validate_ready(), Ok(()));
}

#[test]
fn runtime_from_template_reserves_registered_slot_storage() {
    let mut builder = ExtensionRegistryBuilder::default();
    let template_slot = builder
        .reserve_template_slot("custom", "template", "custom.template")
        .expect("template slot should reserve");
    let battle_slot = builder
        .reserve_battle_slot("custom", "battle", "custom.battle")
        .expect("battle slot should reserve");
    let entity_slot = builder
        .reserve_entity_slot("custom", "entity", "custom.entity")
        .expect("entity slot should reserve");
    let registry = builder.build();
    let mut template = PreparedCombatTemplate::with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], registry);
    template
        .slots
        .set(template_slot, SlotValue::Text("seed".to_owned()))
        .expect("template slot should write");

    let mut runtime = CombatRuntime::from_template(template);
    runtime.slots.set(battle_slot, SlotValue::U64(1)).expect("battle slot should write");
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .slots
        .set(entity_slot, SlotValue::Bool(true))
        .expect("entity slot should write");

    assert_eq!(
        runtime.template_slots.get(template_slot),
        Some(&SlotValue::Text("seed".to_owned()))
    );
    assert_eq!(runtime.slots.get(battle_slot), Some(&SlotValue::U64(1)));
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(entity_slot),
        Some(&SlotValue::Bool(true))
    );
}

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
fn custom_bed2_import_fixture_parses_markers_into_v2_template() {
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
        crate::player::Player::raw_namerena_to_idname("alpha@red+weapon+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}"),
        "alpha@red"
    );
    assert_eq!(facade_bridge.name, "alpha");
    assert_eq!(facade_bridge.team.as_deref(), Some("red"));
    assert_eq!(facade_bridge.hp, 4500);

    let same_team_bridge = CustomBed2Import::parse_player_facade_raw("same@same+bed2[1800]")
        .expect("same-team bed2 raw should bridge through normalized player facade id name");
    assert_eq!(crate::player::Player::raw_namerena_to_idname("same@same+bed2[1800]"), "same");
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
        .expect("mixed legacy/bed2 raw roster should build a prepared template");
    let legacy_storage = crate::engine::storage::Storage::new_arc();
    let mut legacy_plain = crate::player::Player::new_from_namerena_raw("plain@red".to_owned(), legacy_storage)
        .expect("legacy player facade should parse plain player");
    legacy_plain.build();
    let legacy_status = legacy_plain.get_status();

    assert_eq!(template.players.len(), 3);
    assert_eq!(template.players[0].id, 1);
    assert_eq!(template.players[0].name, legacy_plain.id_name());
    assert_eq!(template.players[0].kind, PlayerTemplate::DEFAULT_KIND);
    assert_eq!(template.players[0].team, 0);
    assert_eq!(template.players[0].max_hp, legacy_status.max_hp);
    assert_eq!(template.players[0].attack, legacy_status.attack);
    assert_eq!(template.players[0].defense, legacy_status.defense);
    assert_eq!(template.players[0].resistance, legacy_status.resistance);
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

#[test]
fn runtime_v2_runner_constructs_and_runs_mixed_roster() {
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

    let mut runner = RuntimeV2Runner::from_mixed_roster(&raw_groups, registry, bed2, summon)
        .expect("mixed roster should construct a runtime v2 runner");
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
        rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(1),
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
        actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
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
            delay0: crate::engine::update::DEFAULT_DELAY0_MS,
            delay1: crate::engine::update::DEFAULT_DELAY1_MS,
            update_type: crate::engine::update::UpdateType::None,
        }],
    };

    assert_eq!(strict_diff(&expected, &actual), Ok(()));
}

#[test]
fn runtime_v2_runner_constructs_from_bed2_namerena_raw_fixture_shape() {
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

    let runner = prepared_init_tests::runtime_v2_runner_from_raw(raw_input, |raw_groups| {
        RuntimeV2Runner::from_bed2_roster(raw_groups, registry, bed2, summon)
    });
    let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

    assert_eq!(runner.runtime().entities.len(), 2);
    assert_eq!(runner.runtime().entities.get(EntityIdx(0)).unwrap().template.max_hp, 5);
    assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.max_hp, 8);
    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
    assert_eq!(runner.runtime().rng.i, legacy.randomer.i);
    assert_eq!(runner.runtime().rng.j, legacy.randomer.j);
    assert_eq!(runner.runtime().rng.main_val, legacy.randomer.main_val);
}

#[test]
fn runtime_v2_runner_bed2_raw_can_import_ol_summon_overlay_template_slot() {
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

    let runner = prepared_init_tests::runtime_v2_runner_from_raw(raw_input, |raw_groups| {
        RuntimeV2Runner::from_bed2_roster_with_summon_overlay(
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
    let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
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
fn runtime_v2_runner_bed2_raw_can_import_ol_shadow_overlay_template_slot() {
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

    let runner = prepared_init_tests::runtime_v2_runner_from_raw(raw_input, |raw_groups| {
        RuntimeV2Runner::from_bed2_roster_with_shadow_overlay(
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
    let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
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
fn runtime_v2_runner_bed2_raw_can_import_ol_zombie_overlay_template_slot() {
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

    let runner = prepared_init_tests::runtime_v2_runner_from_raw(raw_input, |raw_groups| {
        RuntimeV2Runner::from_bed2_roster_with_zombie_overlay(
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
    let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
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
fn runtime_v2_runner_bed2_raw_can_import_all_ol_minion_overlay_template_slots() {
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

    let runner = prepared_init_tests::runtime_v2_runner_from_raw(raw_input, |raw_groups| {
        RuntimeV2Runner::from_bed2_roster_with_minion_overlays(
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
    let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
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

#[test]
fn runtime_v2_runner_runs_mixed_namerena_raw_fixture_shape() {
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
    let raw_input = "plain@red\nalpha@red+bed2[9]\n\nseed:custom-seed@!\n\nbeta@blue+bed2[3]\n";

    let mut runner = prepared_init_tests::runtime_v2_runner_from_raw(raw_input, |raw_groups| {
        RuntimeV2Runner::from_mixed_roster(raw_groups, registry, bed2, summon)
    });
    let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");
    runner.runtime_mut().set_skill_handler(summon, skill_noop);
    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
    let initial_rng = crate::runtime_v2::oracle::NormalizedRngCheckpoint::from_runtime(runner.runtime());
    let plain = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.clone();

    let (summary, actual) = runner.run_until_winner_normalized(8);

    assert_eq!(initial_rng.i, legacy.randomer.i);
    assert_eq!(initial_rng.j, legacy.randomer.j);
    assert_eq!(summary.rounds.len(), 2);
    assert_eq!(summary.winner_team, Some(1));
    assert!(!summary.guard_exhausted);
    let expected = NormalizedOutcome {
        winner_team: Some(1),
        round: 2,
        total_score: plain.attack as u64,
        rng: actual.rng.clone(),
        entity_ids: vec![1, 2, 3],
        teams: vec![1, 1, 0],
        hp: vec![plain.max_hp, 9, 0],
        magic_point: vec![plain.magic_point, 0, 0],
        defense: vec![plain.defense, DEFAULT_BED2_DEFENSE, DEFAULT_BED2_DEFENSE],
        resistance: vec![plain.resistance, DEFAULT_BED2_RESISTANCE, DEFAULT_BED2_RESISTANCE],
        alive: vec![true, true, false],
        round_order: vec![0, 1],
        flat_alive: vec![0, 1],
        team_alive: vec![Vec::new(), vec![0, 1]],
        alive_group_count: 1,
        actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
            round: 2,
            actor: 0,
            target: 2,
            amount: plain.attack,
        }],
        frames: vec![NormalizedUpdateFrame {
            message: "[0]攻击[1]".to_owned(),
            caster: 0,
            target: 2,
            targets: Vec::new(),
            param: None,
            score: plain.attack as u32,
            delay0: crate::engine::update::DEFAULT_DELAY0_MS,
            delay1: crate::engine::update::DEFAULT_DELAY1_MS,
            update_type: crate::engine::update::UpdateType::None,
        }],
    };

    assert_eq!(strict_diff(&expected, &actual), Ok(()));
}

#[test]
fn runtime_v2_runner_aligns_large_raw_initial_state_with_legacy_world() {
    let raw_input =
        "虚空托腮 IVHEWTNEA@TigerStar\n\n进口牢货.不可磨灭的回忆之殇 8}i%Yh&<@幻景殇\nseed:2026-03-07 22:54 #013595@!";

    let (runner, legacy) = mixed_raw_runner_for_plain_fixture(raw_input);

    assert_eq!(runner.runtime().entities.len(), 2);
    assert_eq!(legacy.world.all_plr_len(), 2);
    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
    assert_runtime_rng_matches_legacy(runner.runtime(), &legacy);
}

#[test]
#[ignore = "self-referential v2 prefix golden; use legacy/v2 parity report until full plain-player loadout converges"]
fn runtime_v2_runner_large_prefix_normalized_run_matches_golden() {
    let raw_input =
        "虚空托腮 IVHEWTNEA@TigerStar\n\n进口牢货.不可磨灭的回忆之殇 8}i%Yh&<@幻景殇\nseed:2026-03-07 22:54 #013595@!";

    let (mut runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
    let run = runner.run_until_winner_normalized_rounds(4);

    assert_eq!(run.winner_team, None);
    assert!(run.guard_exhausted);
    assert_eq!(run.total_score, 188);
    assert_eq!(run.rounds.len(), 4);
    let expected_rounds = vec![
        NormalizedOutcome {
            winner_team: None,
            round: 1,
            total_score: 57,
            rng: normalized_rng_checkpoint(226, 30),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![350, 265],
            magic_point: vec![28, 29],
            defense: vec![58, 52],
            resistance: vec![49, 57],
            alive: vec![true, true],
            round_order: vec![0, 1],
            flat_alive: vec![0, 1],
            team_alive: vec![vec![0], vec![1]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 1,
                actor: 0,
                target: 1,
                amount: 57,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 57,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
        NormalizedOutcome {
            winner_team: None,
            round: 2,
            total_score: 37,
            rng: normalized_rng_checkpoint(227, 87),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![313, 265],
            magic_point: vec![28, 29],
            defense: vec![58, 52],
            resistance: vec![49, 57],
            alive: vec![true, true],
            round_order: vec![0, 1],
            flat_alive: vec![0, 1],
            team_alive: vec![vec![0], vec![1]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 2,
                actor: 1,
                target: 0,
                amount: 37,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 1,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 37,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
        NormalizedOutcome {
            winner_team: None,
            round: 3,
            total_score: 57,
            rng: normalized_rng_checkpoint(228, 178),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![313, 208],
            magic_point: vec![28, 29],
            defense: vec![58, 52],
            resistance: vec![49, 57],
            alive: vec![true, true],
            round_order: vec![0, 1],
            flat_alive: vec![0, 1],
            team_alive: vec![vec![0], vec![1]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 3,
                actor: 0,
                target: 1,
                amount: 57,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 57,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
        NormalizedOutcome {
            winner_team: None,
            round: 4,
            total_score: 37,
            rng: normalized_rng_checkpoint(229, 251),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![276, 208],
            magic_point: vec![28, 29],
            defense: vec![58, 52],
            resistance: vec![49, 57],
            alive: vec![true, true],
            round_order: vec![0, 1],
            flat_alive: vec![0, 1],
            team_alive: vec![vec![0], vec![1]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 4,
                actor: 1,
                target: 0,
                amount: 37,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 1,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 37,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
    ];

    for (expected, actual) in expected_rounds.iter().zip(&run.rounds) {
        assert_eq!(strict_diff(expected, actual), Ok(()));
    }
}

#[test]
#[ignore = "self-referential v2 terminal golden; use legacy/v2 parity report until full plain-player loadout converges"]
fn runtime_v2_runner_large_full_normalized_run_matches_golden() {
    let raw_input =
        "虚空托腮 IVHEWTNEA@TigerStar\n\n进口牢货.不可磨灭的回忆之殇 8}i%Yh&<@幻景殇\nseed:2026-03-07 22:54 #013595@!";

    let (mut runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
    let run = runner.run_until_winner_normalized_rounds(32);

    assert_eq!(run.winner_team, Some(0));
    assert!(!run.guard_exhausted);
    assert_eq!(run.total_score, 527);
    assert_eq!(run.rounds.len(), 11);

    let expected_rounds = [
        plain_large_expected_round(1, None, 57, 226, 30, [350, 265], [true, true], [0, 1]),
        plain_large_expected_round(2, None, 37, 227, 87, [313, 265], [true, true], [1, 0]),
        plain_large_expected_round(3, None, 57, 228, 178, [313, 208], [true, true], [0, 1]),
        plain_large_expected_round(4, None, 37, 229, 251, [276, 208], [true, true], [1, 0]),
        plain_large_expected_round(5, None, 57, 230, 218, [276, 151], [true, true], [0, 1]),
        plain_large_expected_round(6, None, 37, 231, 61, [239, 151], [true, true], [1, 0]),
        plain_large_expected_round(7, None, 57, 232, 135, [239, 94], [true, true], [0, 1]),
        plain_large_expected_round(8, None, 37, 233, 250, [202, 94], [true, true], [1, 0]),
        plain_large_expected_round(9, None, 57, 234, 242, [202, 37], [true, true], [0, 1]),
        plain_large_expected_round(10, None, 37, 235, 149, [165, 37], [true, true], [1, 0]),
        plain_large_expected_round(11, Some(0), 57, 236, 3, [165, 0], [true, false], [0, 1]),
    ];

    for (expected, actual) in expected_rounds.iter().zip(&run.rounds) {
        assert_eq!(strict_diff(expected, actual), Ok(()));
    }
}

#[test]
fn runtime_v2_runner_aligns_fight_multi_raw_initial_state_with_legacy_world() {
    let raw_input = "测707640862046T，烦恼立刻消失@爱\n坚持 E6b10FVHvKDO@Afterglow\nInfluence #MEZC2wa@Unbound\n耀眼之星 /JxrJYwouGw/@新纪元\n随之任之 #iWZYBGuwxX@🥒\n\n真夜霞 #FBNWDPBPW@无惨\n虚空托腮 UMOXFIARH@TigerStar\nFengshen ONVWTGMPNCKV@nan\nBoundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish\nSpearmaster ZbblyZQQwr@RainWorld_XIV\nseed:1376-2-15@!";

    let (runner, legacy) = mixed_raw_runner_for_plain_fixture(raw_input);

    assert_eq!(runner.runtime().entities.len(), 10);
    assert_eq!(legacy.world.all_plr_len(), 10);
    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
    assert_runtime_rng_matches_legacy(runner.runtime(), &legacy);
}

#[test]
#[ignore = "self-referential v2 prefix golden; use legacy/v2 parity report until full plain-player loadout converges"]
fn runtime_v2_runner_fight_multi_prefix_normalized_run_matches_golden() {
    let raw_input = "测707640862046T，烦恼立刻消失@爱\n坚持 E6b10FVHvKDO@Afterglow\nInfluence #MEZC2wa@Unbound\n耀眼之星 /JxrJYwouGw/@新纪元\n随之任之 #iWZYBGuwxX@🥒\n\n真夜霞 #FBNWDPBPW@无惨\n虚空托腮 UMOXFIARH@TigerStar\nFengshen ONVWTGMPNCKV@nan\nBoundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish\nSpearmaster ZbblyZQQwr@RainWorld_XIV\nseed:1376-2-15@!";

    let (mut runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
    let run = runner.run_until_winner_normalized_rounds(4);

    assert_eq!(run.winner_team, None);
    assert!(run.guard_exhausted);
    assert_eq!(run.total_score, 187);
    assert_eq!(run.rounds.len(), 4);
    let expected_rounds = vec![
        NormalizedOutcome {
            winner_team: None,
            round: 1,
            total_score: 50,
            rng: normalized_rng_checkpoint(215, 67),
            entity_ids: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            teams: vec![1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
            hp: vec![342, 331, 387, 267, 359, 331, 344, 327, 332, 335],
            magic_point: vec![30, 28, 23, 30, 26, 24, 26, 25, 23, 25],
            defense: vec![51, 37, 51, 39, 53, 55, 52, 53, 45, 54],
            resistance: vec![44, 52, 60, 56, 56, 59, 36, 59, 58, 56],
            alive: vec![true, true, true, true, true, true, true, true, true, true],
            round_order: vec![6, 3, 9, 5, 4, 1, 7, 8, 2, 0],
            flat_alive: vec![6, 9, 5, 7, 8, 3, 4, 1, 2, 0],
            team_alive: vec![vec![6, 9, 5, 7, 8], vec![3, 4, 1, 2, 0]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 1,
                actor: 6,
                target: 3,
                amount: 50,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 6,
                target: 3,
                targets: Vec::new(),
                param: None,
                score: 50,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
        NormalizedOutcome {
            winner_team: None,
            round: 2,
            total_score: 20,
            rng: normalized_rng_checkpoint(216, 115),
            entity_ids: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            teams: vec![1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
            hp: vec![342, 331, 387, 267, 359, 331, 324, 327, 332, 335],
            magic_point: vec![30, 28, 23, 30, 26, 24, 26, 25, 23, 25],
            defense: vec![51, 37, 51, 39, 53, 55, 52, 53, 45, 54],
            resistance: vec![44, 52, 60, 56, 56, 59, 36, 59, 58, 56],
            alive: vec![true, true, true, true, true, true, true, true, true, true],
            round_order: vec![6, 3, 9, 5, 4, 1, 7, 8, 2, 0],
            flat_alive: vec![6, 9, 5, 7, 8, 3, 4, 1, 2, 0],
            team_alive: vec![vec![6, 9, 5, 7, 8], vec![3, 4, 1, 2, 0]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 2,
                actor: 3,
                target: 6,
                amount: 20,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 3,
                target: 6,
                targets: Vec::new(),
                param: None,
                score: 20,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
        NormalizedOutcome {
            winner_team: None,
            round: 3,
            total_score: 59,
            rng: normalized_rng_checkpoint(217, 59),
            entity_ids: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            teams: vec![1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
            hp: vec![342, 331, 387, 208, 359, 331, 324, 327, 332, 335],
            magic_point: vec![30, 28, 23, 30, 26, 24, 26, 25, 23, 25],
            defense: vec![51, 37, 51, 39, 53, 55, 52, 53, 45, 54],
            resistance: vec![44, 52, 60, 56, 56, 59, 36, 59, 58, 56],
            alive: vec![true, true, true, true, true, true, true, true, true, true],
            round_order: vec![6, 3, 9, 5, 4, 1, 7, 8, 2, 0],
            flat_alive: vec![6, 9, 5, 7, 8, 3, 4, 1, 2, 0],
            team_alive: vec![vec![6, 9, 5, 7, 8], vec![3, 4, 1, 2, 0]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 3,
                actor: 9,
                target: 3,
                amount: 59,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 9,
                target: 3,
                targets: Vec::new(),
                param: None,
                score: 59,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
        NormalizedOutcome {
            winner_team: None,
            round: 4,
            total_score: 58,
            rng: normalized_rng_checkpoint(218, 78),
            entity_ids: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            teams: vec![1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
            hp: vec![342, 331, 387, 150, 359, 331, 324, 327, 332, 335],
            magic_point: vec![30, 28, 23, 30, 26, 24, 26, 25, 23, 25],
            defense: vec![51, 37, 51, 39, 53, 55, 52, 53, 45, 54],
            resistance: vec![44, 52, 60, 56, 56, 59, 36, 59, 58, 56],
            alive: vec![true, true, true, true, true, true, true, true, true, true],
            round_order: vec![6, 3, 9, 5, 4, 1, 7, 8, 2, 0],
            flat_alive: vec![6, 9, 5, 7, 8, 3, 4, 1, 2, 0],
            team_alive: vec![vec![6, 9, 5, 7, 8], vec![3, 4, 1, 2, 0]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 4,
                actor: 5,
                target: 3,
                amount: 58,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 5,
                target: 3,
                targets: Vec::new(),
                param: None,
                score: 58,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
    ];

    for (expected, actual) in expected_rounds.iter().zip(&run.rounds) {
        assert_eq!(strict_diff(expected, actual), Ok(()));
    }
}

#[test]
fn runtime_v2_runner_fight_multi_reports_real_legacy_divergence() {
    let raw_input = "测707640862046T，烦恼立刻消失@爱\n坚持 E6b10FVHvKDO@Afterglow\nInfluence #MEZC2wa@Unbound\n耀眼之星 /JxrJYwouGw/@新纪元\n随之任之 #iWZYBGuwxX@🥒\n\n真夜霞 #FBNWDPBPW@无惨\n虚空托腮 UMOXFIARH@TigerStar\nFengshen ONVWTGMPNCKV@nan\nBoundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish\nSpearmaster ZbblyZQQwr@RainWorld_XIV\nseed:1376-2-15@!";

    let mut legacy_runner =
        crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy fight_multi should construct");
    let legacy = normalize_legacy_run(&mut legacy_runner, 256);
    let (mut v2_runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
    let v2 = v2_runner.run_until_winner_normalized_rounds(256);

    assert_eq!(legacy.rounds.len(), 84);
    assert_eq!(legacy.total_score, 6766);
    assert!(matches!(strict_diff_runs(&legacy, &v2), Err(StrictRunDiff::Round { .. })));
}

#[test]
#[ignore = "self-referential v2 terminal golden; use legacy/v2 parity report until behavior converges"]
fn runtime_v2_runner_fight_multi_full_terminal_normalized_run_matches_golden() {
    let raw_input = "测707640862046T，烦恼立刻消失@爱\n坚持 E6b10FVHvKDO@Afterglow\nInfluence #MEZC2wa@Unbound\n耀眼之星 /JxrJYwouGw/@新纪元\n随之任之 #iWZYBGuwxX@🥒\n\n真夜霞 #FBNWDPBPW@无惨\n虚空托腮 UMOXFIARH@TigerStar\nFengshen ONVWTGMPNCKV@nan\nBoundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish\nSpearmaster ZbblyZQQwr@RainWorld_XIV\nseed:1376-2-15@!";

    let (mut runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
    let run = runner.run_until_winner_normalized_rounds(256);

    assert_eq!(run.winner_team, Some(1));
    assert!(!run.guard_exhausted);
    assert_eq!(run.total_score, 3211);
    assert_eq!(run.rounds.len(), 88);

    let expected_checkpoints = [
        (
            1,
            None,
            50,
            215,
            67,
            [342, 331, 387, 267, 359, 331, 344, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [6, 3, 50],
        ),
        (
            2,
            None,
            20,
            216,
            115,
            [342, 331, 387, 267, 359, 331, 324, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [3, 6, 20],
        ),
        (
            3,
            None,
            59,
            217,
            59,
            [342, 331, 387, 208, 359, 331, 324, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [9, 3, 59],
        ),
        (
            4,
            None,
            58,
            218,
            78,
            [342, 331, 387, 150, 359, 331, 324, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [5, 3, 58],
        ),
        (
            5,
            None,
            48,
            219,
            152,
            [342, 331, 387, 150, 359, 331, 276, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [4, 6, 48],
        ),
        (
            6,
            None,
            47,
            220,
            55,
            [342, 331, 387, 150, 359, 331, 229, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [1, 6, 47],
        ),
        (
            7,
            None,
            37,
            221,
            90,
            [342, 331, 387, 113, 359, 331, 229, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [7, 3, 37],
        ),
        (
            8,
            None,
            5,
            222,
            110,
            [342, 331, 387, 108, 359, 331, 229, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [8, 3, 5],
        ),
        (
            9,
            None,
            61,
            223,
            126,
            [342, 331, 387, 108, 359, 331, 168, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [2, 6, 61],
        ),
        (
            10,
            None,
            20,
            224,
            62,
            [342, 331, 387, 108, 359, 331, 148, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [0, 6, 20],
        ),
        (
            11,
            None,
            50,
            225,
            11,
            [342, 331, 387, 58, 359, 331, 148, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [6, 3, 50],
        ),
        (
            12,
            None,
            20,
            226,
            147,
            [342, 331, 387, 58, 359, 331, 128, 327, 332, 335],
            [true, true, true, true, true, true, true, true, true, true],
            [3, 6, 20],
        ),
        (
            13,
            None,
            59,
            227,
            2,
            [342, 331, 387, 0, 359, 331, 128, 327, 332, 335],
            [true, true, true, false, true, true, true, true, true, true],
            [9, 3, 59],
        ),
        (
            14,
            None,
            58,
            228,
            44,
            [342, 331, 387, 0, 301, 331, 128, 327, 332, 335],
            [true, true, true, false, true, true, true, true, true, true],
            [5, 4, 58],
        ),
        (
            15,
            None,
            48,
            229,
            76,
            [342, 331, 387, 0, 301, 331, 80, 327, 332, 335],
            [true, true, true, false, true, true, true, true, true, true],
            [4, 6, 48],
        ),
        (
            16,
            None,
            47,
            230,
            217,
            [342, 331, 387, 0, 301, 331, 33, 327, 332, 335],
            [true, true, true, false, true, true, true, true, true, true],
            [1, 6, 47],
        ),
        (
            17,
            None,
            37,
            231,
            11,
            [342, 331, 387, 0, 264, 331, 33, 327, 332, 335],
            [true, true, true, false, true, true, true, true, true, true],
            [7, 4, 37],
        ),
        (
            18,
            None,
            5,
            232,
            198,
            [342, 331, 387, 0, 259, 331, 33, 327, 332, 335],
            [true, true, true, false, true, true, true, true, true, true],
            [8, 4, 5],
        ),
        (
            19,
            None,
            61,
            233,
            235,
            [342, 331, 387, 0, 259, 331, 0, 327, 332, 335],
            [true, true, true, false, true, true, false, true, true, true],
            [2, 6, 61],
        ),
        (
            20,
            None,
            20,
            234,
            89,
            [342, 331, 387, 0, 259, 331, 0, 327, 332, 315],
            [true, true, true, false, true, true, false, true, true, true],
            [0, 9, 20],
        ),
        (
            21,
            None,
            59,
            235,
            126,
            [342, 331, 387, 0, 200, 331, 0, 327, 332, 315],
            [true, true, true, false, true, true, false, true, true, true],
            [9, 4, 59],
        ),
        (
            22,
            None,
            58,
            236,
            92,
            [342, 331, 387, 0, 142, 331, 0, 327, 332, 315],
            [true, true, true, false, true, true, false, true, true, true],
            [5, 4, 58],
        ),
        (
            23,
            None,
            48,
            237,
            176,
            [342, 331, 387, 0, 142, 331, 0, 327, 332, 267],
            [true, true, true, false, true, true, false, true, true, true],
            [4, 9, 48],
        ),
        (
            24,
            None,
            47,
            238,
            119,
            [342, 331, 387, 0, 142, 331, 0, 327, 332, 220],
            [true, true, true, false, true, true, false, true, true, true],
            [1, 9, 47],
        ),
        (
            25,
            None,
            37,
            239,
            54,
            [342, 331, 387, 0, 105, 331, 0, 327, 332, 220],
            [true, true, true, false, true, true, false, true, true, true],
            [7, 4, 37],
        ),
        (
            26,
            None,
            5,
            240,
            168,
            [342, 331, 387, 0, 100, 331, 0, 327, 332, 220],
            [true, true, true, false, true, true, false, true, true, true],
            [8, 4, 5],
        ),
        (
            27,
            None,
            61,
            241,
            169,
            [342, 331, 387, 0, 100, 331, 0, 327, 332, 159],
            [true, true, true, false, true, true, false, true, true, true],
            [2, 9, 61],
        ),
        (
            28,
            None,
            20,
            242,
            84,
            [342, 331, 387, 0, 100, 331, 0, 327, 332, 139],
            [true, true, true, false, true, true, false, true, true, true],
            [0, 9, 20],
        ),
        (
            29,
            None,
            59,
            243,
            173,
            [342, 331, 387, 0, 41, 331, 0, 327, 332, 139],
            [true, true, true, false, true, true, false, true, true, true],
            [9, 4, 59],
        ),
        (
            30,
            None,
            58,
            244,
            241,
            [342, 331, 387, 0, 0, 331, 0, 327, 332, 139],
            [true, true, true, false, false, true, false, true, true, true],
            [5, 4, 58],
        ),
        (
            31,
            None,
            47,
            245,
            186,
            [342, 331, 387, 0, 0, 331, 0, 327, 332, 92],
            [true, true, true, false, false, true, false, true, true, true],
            [1, 9, 47],
        ),
        (
            32,
            None,
            37,
            246,
            197,
            [342, 294, 387, 0, 0, 331, 0, 327, 332, 92],
            [true, true, true, false, false, true, false, true, true, true],
            [7, 1, 37],
        ),
        (
            33,
            None,
            5,
            247,
            40,
            [342, 289, 387, 0, 0, 331, 0, 327, 332, 92],
            [true, true, true, false, false, true, false, true, true, true],
            [8, 1, 5],
        ),
        (
            34,
            None,
            61,
            248,
            9,
            [342, 289, 387, 0, 0, 331, 0, 327, 332, 31],
            [true, true, true, false, false, true, false, true, true, true],
            [2, 9, 61],
        ),
        (
            35,
            None,
            20,
            249,
            136,
            [342, 289, 387, 0, 0, 331, 0, 327, 332, 11],
            [true, true, true, false, false, true, false, true, true, true],
            [0, 9, 20],
        ),
        (
            36,
            None,
            59,
            250,
            211,
            [342, 230, 387, 0, 0, 331, 0, 327, 332, 11],
            [true, true, true, false, false, true, false, true, true, true],
            [9, 1, 59],
        ),
        (
            37,
            None,
            58,
            251,
            207,
            [342, 172, 387, 0, 0, 331, 0, 327, 332, 11],
            [true, true, true, false, false, true, false, true, true, true],
            [5, 1, 58],
        ),
        (
            38,
            None,
            47,
            252,
            245,
            [342, 172, 387, 0, 0, 331, 0, 327, 332, 0],
            [true, true, true, false, false, true, false, true, true, false],
            [1, 9, 47],
        ),
        (
            39,
            None,
            37,
            253,
            82,
            [342, 135, 387, 0, 0, 331, 0, 327, 332, 0],
            [true, true, true, false, false, true, false, true, true, false],
            [7, 1, 37],
        ),
        (
            40,
            None,
            5,
            254,
            138,
            [342, 130, 387, 0, 0, 331, 0, 327, 332, 0],
            [true, true, true, false, false, true, false, true, true, false],
            [8, 1, 5],
        ),
        (
            41,
            None,
            61,
            255,
            140,
            [342, 130, 387, 0, 0, 270, 0, 327, 332, 0],
            [true, true, true, false, false, true, false, true, true, false],
            [2, 5, 61],
        ),
        (
            42,
            None,
            20,
            0,
            50,
            [342, 130, 387, 0, 0, 250, 0, 327, 332, 0],
            [true, true, true, false, false, true, false, true, true, false],
            [0, 5, 20],
        ),
        (
            43,
            None,
            58,
            1,
            43,
            [342, 72, 387, 0, 0, 250, 0, 327, 332, 0],
            [true, true, true, false, false, true, false, true, true, false],
            [5, 1, 58],
        ),
        (
            44,
            None,
            47,
            2,
            154,
            [342, 72, 387, 0, 0, 203, 0, 327, 332, 0],
            [true, true, true, false, false, true, false, true, true, false],
            [1, 5, 47],
        ),
        (
            45,
            None,
            37,
            3,
            188,
            [342, 35, 387, 0, 0, 203, 0, 327, 332, 0],
            [true, true, true, false, false, true, false, true, true, false],
            [7, 1, 37],
        ),
        (
            46,
            None,
            5,
            4,
            57,
            [342, 30, 387, 0, 0, 203, 0, 327, 332, 0],
            [true, true, true, false, false, true, false, true, true, false],
            [8, 1, 5],
        ),
        (
            47,
            None,
            61,
            5,
            240,
            [342, 30, 387, 0, 0, 142, 0, 327, 332, 0],
            [true, true, true, false, false, true, false, true, true, false],
            [2, 5, 61],
        ),
        (
            48,
            None,
            20,
            6,
            222,
            [342, 30, 387, 0, 0, 122, 0, 327, 332, 0],
            [true, true, true, false, false, true, false, true, true, false],
            [0, 5, 20],
        ),
        (
            49,
            None,
            58,
            7,
            58,
            [342, 0, 387, 0, 0, 122, 0, 327, 332, 0],
            [true, false, true, false, false, true, false, true, true, false],
            [5, 1, 58],
        ),
        (
            50,
            None,
            37,
            8,
            46,
            [342, 0, 350, 0, 0, 122, 0, 327, 332, 0],
            [true, false, true, false, false, true, false, true, true, false],
            [7, 2, 37],
        ),
        (
            51,
            None,
            5,
            9,
            15,
            [342, 0, 345, 0, 0, 122, 0, 327, 332, 0],
            [true, false, true, false, false, true, false, true, true, false],
            [8, 2, 5],
        ),
        (
            52,
            None,
            61,
            10,
            42,
            [342, 0, 345, 0, 0, 61, 0, 327, 332, 0],
            [true, false, true, false, false, true, false, true, true, false],
            [2, 5, 61],
        ),
        (
            53,
            None,
            20,
            11,
            92,
            [342, 0, 345, 0, 0, 41, 0, 327, 332, 0],
            [true, false, true, false, false, true, false, true, true, false],
            [0, 5, 20],
        ),
        (
            54,
            None,
            58,
            12,
            105,
            [342, 0, 287, 0, 0, 41, 0, 327, 332, 0],
            [true, false, true, false, false, true, false, true, true, false],
            [5, 2, 58],
        ),
        (
            55,
            None,
            37,
            13,
            205,
            [342, 0, 250, 0, 0, 41, 0, 327, 332, 0],
            [true, false, true, false, false, true, false, true, true, false],
            [7, 2, 37],
        ),
        (
            56,
            None,
            5,
            14,
            86,
            [342, 0, 245, 0, 0, 41, 0, 327, 332, 0],
            [true, false, true, false, false, true, false, true, true, false],
            [8, 2, 5],
        ),
        (
            57,
            None,
            61,
            15,
            55,
            [342, 0, 245, 0, 0, 0, 0, 327, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [2, 5, 61],
        ),
        (
            58,
            None,
            20,
            16,
            186,
            [342, 0, 245, 0, 0, 0, 0, 307, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [0, 7, 20],
        ),
        (
            59,
            None,
            37,
            17,
            58,
            [342, 0, 208, 0, 0, 0, 0, 307, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [7, 2, 37],
        ),
        (
            60,
            None,
            5,
            18,
            239,
            [342, 0, 203, 0, 0, 0, 0, 307, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [8, 2, 5],
        ),
        (
            61,
            None,
            61,
            19,
            84,
            [342, 0, 203, 0, 0, 0, 0, 246, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [2, 7, 61],
        ),
        (
            62,
            None,
            20,
            20,
            196,
            [342, 0, 203, 0, 0, 0, 0, 226, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [0, 7, 20],
        ),
        (
            63,
            None,
            37,
            21,
            219,
            [342, 0, 166, 0, 0, 0, 0, 226, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [7, 2, 37],
        ),
        (
            64,
            None,
            5,
            22,
            22,
            [342, 0, 161, 0, 0, 0, 0, 226, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [8, 2, 5],
        ),
        (
            65,
            None,
            61,
            23,
            250,
            [342, 0, 161, 0, 0, 0, 0, 165, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [2, 7, 61],
        ),
        (
            66,
            None,
            20,
            24,
            57,
            [342, 0, 161, 0, 0, 0, 0, 145, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [0, 7, 20],
        ),
        (
            67,
            None,
            37,
            25,
            36,
            [342, 0, 124, 0, 0, 0, 0, 145, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [7, 2, 37],
        ),
        (
            68,
            None,
            5,
            26,
            112,
            [342, 0, 119, 0, 0, 0, 0, 145, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [8, 2, 5],
        ),
        (
            69,
            None,
            61,
            27,
            215,
            [342, 0, 119, 0, 0, 0, 0, 84, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [2, 7, 61],
        ),
        (
            70,
            None,
            20,
            28,
            239,
            [342, 0, 119, 0, 0, 0, 0, 64, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [0, 7, 20],
        ),
        (
            71,
            None,
            37,
            29,
            214,
            [342, 0, 82, 0, 0, 0, 0, 64, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [7, 2, 37],
        ),
        (
            72,
            None,
            5,
            30,
            1,
            [342, 0, 77, 0, 0, 0, 0, 64, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [8, 2, 5],
        ),
        (
            73,
            None,
            61,
            31,
            56,
            [342, 0, 77, 0, 0, 0, 0, 3, 332, 0],
            [true, false, true, false, false, false, false, true, true, false],
            [2, 7, 61],
        ),
        (
            74,
            None,
            20,
            32,
            161,
            [342, 0, 77, 0, 0, 0, 0, 0, 332, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [0, 7, 20],
        ),
        (
            75,
            None,
            5,
            33,
            208,
            [342, 0, 72, 0, 0, 0, 0, 0, 332, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [8, 2, 5],
        ),
        (
            76,
            None,
            61,
            34,
            164,
            [342, 0, 72, 0, 0, 0, 0, 0, 271, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [2, 8, 61],
        ),
        (
            77,
            None,
            20,
            35,
            128,
            [342, 0, 72, 0, 0, 0, 0, 0, 251, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [0, 8, 20],
        ),
        (
            78,
            None,
            5,
            36,
            107,
            [342, 0, 67, 0, 0, 0, 0, 0, 251, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [8, 2, 5],
        ),
        (
            79,
            None,
            61,
            37,
            115,
            [342, 0, 67, 0, 0, 0, 0, 0, 190, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [2, 8, 61],
        ),
        (
            80,
            None,
            20,
            38,
            6,
            [342, 0, 67, 0, 0, 0, 0, 0, 170, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [0, 8, 20],
        ),
        (
            81,
            None,
            5,
            39,
            57,
            [342, 0, 62, 0, 0, 0, 0, 0, 170, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [8, 2, 5],
        ),
        (
            82,
            None,
            61,
            40,
            156,
            [342, 0, 62, 0, 0, 0, 0, 0, 109, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [2, 8, 61],
        ),
        (
            83,
            None,
            20,
            41,
            137,
            [342, 0, 62, 0, 0, 0, 0, 0, 89, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [0, 8, 20],
        ),
        (
            84,
            None,
            5,
            42,
            164,
            [342, 0, 57, 0, 0, 0, 0, 0, 89, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [8, 2, 5],
        ),
        (
            85,
            None,
            61,
            43,
            157,
            [342, 0, 57, 0, 0, 0, 0, 0, 28, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [2, 8, 61],
        ),
        (
            86,
            None,
            20,
            44,
            199,
            [342, 0, 57, 0, 0, 0, 0, 0, 8, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [0, 8, 20],
        ),
        (
            87,
            None,
            5,
            45,
            5,
            [342, 0, 52, 0, 0, 0, 0, 0, 8, 0],
            [true, false, true, false, false, false, false, false, true, false],
            [8, 2, 5],
        ),
        (
            88,
            Some(1),
            61,
            46,
            249,
            [342, 0, 52, 0, 0, 0, 0, 0, 0, 0],
            [true, false, true, false, false, false, false, false, false, false],
            [2, 8, 61],
        ),
    ];
    assert_eq!(run.rounds.len(), expected_checkpoints.len());
    for (round, (expected_round, expected_winner, expected_score, rng_i, rng_j, expected_hp, expected_alive, expected_action)) in
        run.rounds.iter().zip(expected_checkpoints)
    {
        assert_eq!(round.round, expected_round);
        assert_eq!(round.winner_team, expected_winner);
        assert_eq!(round.total_score, expected_score);
        assert_eq!(round.rng, normalized_rng_checkpoint(rng_i, rng_j));
        assert_eq!(round.hp, expected_hp);
        assert_eq!(round.alive, expected_alive);
        assert_eq!(round.actions.len(), 1);
        assert_eq!(round.frames.len(), 1);
        let action = round.actions.first().unwrap();
        let frame = round.frames.first().unwrap();
        assert_eq!([action.actor, action.target, action.amount as usize], expected_action);
        assert_eq!([frame.caster, frame.target, frame.score as usize], expected_action);
    }

    let expected_terminal = NormalizedOutcome {
        winner_team: Some(1),
        round: 88,
        total_score: 61,
        rng: normalized_rng_checkpoint(46, 249),
        entity_ids: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        teams: vec![1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
        hp: vec![342, 0, 52, 0, 0, 0, 0, 0, 0, 0],
        magic_point: vec![30, 28, 23, 30, 26, 24, 26, 25, 23, 25],
        defense: vec![51, 37, 51, 39, 53, 55, 52, 53, 45, 54],
        resistance: vec![44, 52, 60, 56, 56, 59, 36, 59, 58, 56],
        alive: vec![true, false, true, false, false, false, false, false, false, false],
        round_order: vec![6, 3, 9, 5, 4, 1, 7, 8, 2, 0],
        flat_alive: vec![2, 0],
        team_alive: vec![vec![], vec![2, 0]],
        alive_group_count: 1,
        actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
            round: 88,
            actor: 2,
            target: 8,
            amount: 61,
        }],
        frames: vec![NormalizedUpdateFrame {
            message: "[0]攻击[1]".to_owned(),
            caster: 2,
            target: 8,
            targets: Vec::new(),
            param: None,
            score: 61,
            delay0: crate::engine::update::DEFAULT_DELAY0_MS,
            delay1: crate::engine::update::DEFAULT_DELAY1_MS,
            update_type: crate::engine::update::UpdateType::None,
        }],
    };

    assert_eq!(strict_diff(&expected_terminal, run.rounds.last().unwrap()), Ok(()));
}

#[test]
fn runtime_v2_runner_rejects_plain_rows_in_bed2_roster() {
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
            PlayerKindPolicies::default(),
        )
        .expect("bed2 kind should register");
    let registry = builder.build();
    let raw_groups = vec![vec!["plain".to_owned()], vec!["beta@blue@bed2".to_owned()]];

    let err = RuntimeV2Runner::from_bed2_roster(&raw_groups, registry, bed2, summon)
        .expect_err("bed2-only runner constructor should reject non-bed2 rows");

    assert_eq!(
        err,
        CustomBed2RosterImportError {
            team_index: 0,
            player_index: 0,
            raw: "plain".to_owned(),
        }
    );
}

#[test]
fn runtime_v2_runner_runs_mixed_roster_until_winner() {
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
        vec!["seed:custom-seed@!".to_owned(), "beta@blue+bed2[3]".to_owned()],
    ];

    let mut runner = RuntimeV2Runner::from_mixed_roster(&raw_groups, registry, bed2, summon)
        .expect("mixed roster should construct a runtime v2 runner");
    runner.runtime_mut().set_skill_handler(summon, skill_noop);
    let plain = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.clone();

    let (summary, actual) = runner.run_until_winner_normalized(8);

    assert_eq!(summary.rounds.len(), 1);
    assert_eq!(summary.winner_team, Some(0));
    assert!(!summary.guard_exhausted);
    let expected = NormalizedOutcome {
        winner_team: Some(0),
        round: 1,
        total_score: plain.attack as u64,
        rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(1),
        entity_ids: vec![1, 2, 3],
        teams: vec![0, 0, 1],
        hp: vec![plain.max_hp, 9, 0],
        magic_point: vec![plain.magic_point, 0, 0],
        defense: vec![plain.defense, DEFAULT_BED2_DEFENSE, DEFAULT_BED2_DEFENSE],
        resistance: vec![plain.resistance, DEFAULT_BED2_RESISTANCE, DEFAULT_BED2_RESISTANCE],
        alive: vec![true, true, false],
        round_order: vec![0, 1],
        flat_alive: vec![0, 1],
        team_alive: vec![vec![0, 1], Vec::new()],
        alive_group_count: 1,
        actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
            round: 1,
            actor: 0,
            target: 2,
            amount: plain.attack,
        }],
        frames: vec![NormalizedUpdateFrame {
            message: "[0]攻击[1]".to_owned(),
            caster: 0,
            target: 2,
            targets: Vec::new(),
            param: None,
            score: plain.attack as u32,
            delay0: crate::engine::update::DEFAULT_DELAY0_MS,
            delay1: crate::engine::update::DEFAULT_DELAY1_MS,
            update_type: crate::engine::update::UpdateType::None,
        }],
    };

    assert_eq!(strict_diff(&expected, &actual), Ok(()));
}

#[test]
fn custom_runner_multi_round_normalized_run_matches_strict_diff_golden() {
    let mut builder = ExtensionRegistryBuilder::default();
    let bed2 = builder
        .register_player_kind_with_policies(
            "custom",
            "bed2-runner",
            "custom.bed2_runner",
            PlayerKindFlags::BED2,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: false,
            },
        )
        .expect("bed2 runner kind should register");
    let registry = builder.build();
    let mut runner = RuntimeV2Runner::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 8, 3),
            PlayerTemplate::with_kind(2, "bed2", bed2, 1, 5, 0).with_def_res(DEFAULT_BED2_DEFENSE, DEFAULT_BED2_RESISTANCE),
        ],
        registry,
    ));

    let run = runner.run_until_winner_normalized_rounds(8);

    assert_eq!(run.winner_team, Some(0));
    assert!(!run.guard_exhausted);
    assert_eq!(run.total_score, 6);
    assert_eq!(run.rounds.len(), 3);
    let expected_rounds = vec![
        NormalizedOutcome {
            winner_team: None,
            round: 1,
            total_score: 3,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(1),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![8, 2],
            magic_point: vec![0, 0],
            defense: vec![0, DEFAULT_BED2_DEFENSE],
            resistance: vec![0, DEFAULT_BED2_RESISTANCE],
            alive: vec![true, true],
            round_order: vec![0, 1],
            flat_alive: vec![0, 1],
            team_alive: vec![vec![0], vec![1]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 1,
                actor: 0,
                target: 1,
                amount: 3,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 3,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
        NormalizedOutcome {
            winner_team: None,
            round: 2,
            total_score: 0,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(2),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![8, 2],
            magic_point: vec![0, 0],
            defense: vec![0, DEFAULT_BED2_DEFENSE],
            resistance: vec![0, DEFAULT_BED2_RESISTANCE],
            alive: vec![true, true],
            round_order: vec![0, 1],
            flat_alive: vec![0, 1],
            team_alive: vec![vec![0], vec![1]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 2,
                actor: 1,
                target: 0,
                amount: 0,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 1,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
        NormalizedOutcome {
            winner_team: Some(0),
            round: 3,
            total_score: 3,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(3),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![8, 0],
            magic_point: vec![0, 0],
            defense: vec![0, DEFAULT_BED2_DEFENSE],
            resistance: vec![0, DEFAULT_BED2_RESISTANCE],
            alive: vec![true, false],
            round_order: vec![0],
            flat_alive: vec![0],
            team_alive: vec![vec![0], Vec::new()],
            alive_group_count: 1,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 3,
                actor: 0,
                target: 1,
                amount: 3,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 3,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        },
    ];

    for (expected, actual) in expected_rounds.iter().zip(&run.rounds) {
        assert_eq!(strict_diff(expected, actual), Ok(()));
    }
}

#[cfg(not(feature = "no_debug"))]
#[test]
fn run_minimal_round_records_trace_when_enabled() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.enable_trace();

    runtime.run_minimal_round();

    let trace = runtime.trace().expect("trace should be enabled");
    assert_eq!(trace.actions.len(), 1);
    assert_eq!(trace.actions[0].actor, EntityIdx(0));
    assert_eq!(trace.actions[0].target, EntityIdx(1));
    assert_eq!(
        trace.actions[0].rng_before,
        Some(RngCheckpoint {
            i: 0,
            j: 0,
            byte_count: 0,
        })
    );
    assert_eq!(
        trace.actions[0].rng_after,
        Some(RngCheckpoint {
            i: 1,
            j: 1,
            byte_count: 0,
        })
    );
    assert_eq!(trace.frames.len(), 1);
    assert_eq!(trace.frames[0].total_score, 3);
    assert_eq!(trace.frames[0].winner_team, None);
    assert_eq!(
        trace.frames[0].rng_after,
        Some(RngCheckpoint {
            i: 1,
            j: 1,
            byte_count: 0,
        })
    );
    assert_eq!(trace.frames[0].updates[0].score, 3);
}

#[test]
fn run_minimal_round_applies_damage_frame() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    let outcome = runtime.run_minimal_round();

    assert_eq!(outcome.winner_team, None);
    assert!(outcome.frame.is_some());
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    assert_eq!(runtime.round, 1);
}

#[test]
fn finish_round_advances_round_without_action_or_frame() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));

    let outcome = runtime.finish_round(None, RunUpdates::new());

    assert!(outcome.action.is_none());
    assert!(outcome.frame.is_none());
    assert_eq!(outcome.winner_team, None);
    assert_eq!(runtime.round, 1);
}

#[test]
fn run_minimal_round_reports_winner_after_lethal_damage() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 3, 3));
    let outcome = runtime.run_minimal_round();

    assert_eq!(outcome.winner_team, Some(0));
    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
}

#[test]
fn flush_effects_routes_root_owner_damage_to_owner_entity() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
    });
    runtime.flush_effects().expect("spawn should emit update");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });

    let frame = runtime.flush_effects().expect("routed damage should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[0].score, 4);
}

#[test]
fn flush_effects_runs_die_hook_on_resolved_root_owner() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon kind should register");
    let die_state = builder
        .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
        .expect("die state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 4, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 99,
        extension_state_id: Some(die_state),
        hook_mask: ProcMask::DIE,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(die_state, state_marks_update);
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
    });
    runtime.flush_effects().expect("spawn should emit update");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });

    let frame = runtime.flush_effects().expect("lethal routed damage should emit hooks");

    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[1].message, "state mark");
    assert_eq!(frame.updates.updates[1].score, 99);
}

#[test]
fn flush_effects_shares_summon_damage_to_owner_entity() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
    });
    runtime.flush_effects().expect("spawn should emit update");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });

    let frame = runtime.flush_effects().expect("shared damage should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 1);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(frame.updates.updates[0].score, 4);
    assert_eq!(frame.updates.updates[1].target, 0);
    assert_eq!(frame.updates.updates[1].score, 4);
}

#[test]
fn flush_effects_runs_die_hook_on_damage_share_owner() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon kind should register");
    let die_state = builder
        .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
        .expect("die state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 4, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 101,
        extension_state_id: Some(die_state),
        hook_mask: ProcMask::DIE,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(die_state, state_marks_update);
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
    });
    runtime.flush_effects().expect("spawn should emit update");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });

    let frame = runtime.flush_effects().expect("shared lethal damage should emit hooks");

    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(frame.updates.updates[1].target, 0);
    assert_eq!(frame.updates.updates[2].message, "state mark");
    assert_eq!(frame.updates.updates[2].score, 101);
}

#[test]
fn flush_effects_removes_lethal_damage_target_from_alive_views() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 4, 3));
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        amount: 4,
    });

    runtime.flush_effects().expect("lethal damage should emit update");

    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    assert_eq!(runtime.world.team_alive(1), Some([].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0)]);
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert_eq!(runtime.world.first_alive_enemy(EntityIdx(0), &runtime.entities), None);
}

#[test]
fn flush_effects_shares_owner_damage_to_alive_summons() {
    let mut builder = ExtensionRegistryBuilder::default();
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "owner",
            "custom.owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("owner kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::new(3, "summon-a", 0, 5, 1),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::new(4, "summon-b", 0, 5, 1),
    });
    runtime.flush_effects().expect("spawns should emit updates");
    runtime.entities.get_mut(EntityIdx(3)).unwrap().runtime.alive = false;
    runtime.entities.get_mut(EntityIdx(3)).unwrap().runtime.hp = 0;
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(0),
        amount: 3,
    });

    let frame = runtime.flush_effects().expect("shared summon damage should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 7);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 2);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[0].score, 3);
    assert_eq!(frame.updates.updates[1].target, 2);
    assert_eq!(frame.updates.updates[1].score, 3);
}

#[test]
fn custom_summon_fixture_combines_owner_route_share_and_skill_reuse() {
    let mut builder = ExtensionRegistryBuilder::default();
    let recast_skill = builder
        .register_skill(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon-owner",
            "custom.summon_owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1).with_skills([recast_skill]),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "summon-recast", summon_kind, 0, 5, 1).with_skills([recast_skill]),
    });
    runtime.flush_effects().expect("summon spawns should emit updates");

    assert_eq!(
        runtime.entities.get(EntityIdx(2)).unwrap().template.skills.skills(),
        &[recast_skill]
    );
    assert_eq!(
        runtime.entities.get(EntityIdx(3)).unwrap().template.skills.skills(),
        &[recast_skill]
    );
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.owner, EntityIdx(0));
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.root_owner, EntityIdx(0));
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.owner, EntityIdx(0));
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.root_owner, EntityIdx(0));

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });
    let routed = runtime.flush_effects().expect("summon/root owner damage should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 5);
    assert_eq!(routed.updates.updates.len(), 1);
    assert_eq!(routed.updates.updates[0].target, 0);
    assert_eq!(routed.updates.updates[0].score, 4);

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(0),
        amount: 2,
    });
    let shared = runtime.flush_effects().expect("owner damage should share to summons");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 4);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 3);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 3);
    assert_eq!(shared.updates.updates.len(), 3);
    assert_eq!(shared.updates.updates[0].target, 0);
    assert_eq!(shared.updates.updates[1].target, 2);
    assert_eq!(shared.updates.updates[2].target, 3);
}

#[test]
fn custom_summon_fixture_inherits_owner_defense_and_resistance() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_def_res(77, 88),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));

    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 10, 1).with_def_res(11, 22),
    });
    runtime.flush_effects().expect("summon spawn should emit update");

    let summon = runtime.entities.get(EntityIdx(2)).expect("summon should spawn");
    assert_eq!(summon.template.defense, 77);
    assert_eq!(summon.template.resistance, 88);
    assert_eq!(summon.runtime.defense, 77);
    assert_eq!(summon.runtime.resistance, 88);
}

#[test]
fn summon_default_skill_loadout_keeps_fixed_lanes_and_active_order() {
    let fire = SkillId(11);
    let explode = SkillId(12);
    let loadout = summon_default_skill_loadout(fire, explode, [2, 0, 1]);

    assert_eq!(loadout.skills(), &[fire, fire, explode]);
    assert_eq!(loadout.active_order(), &[2, 0, 1]);
}

#[test]
fn charged_summon_template_can_disable_share_damage() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "charged-summon", summon_kind, 0, 5, 1)
            .with_damage_share_policy(DamageSharePolicy::None)
            .with_speed_points(2048),
    });
    runtime.flush_effects().expect("charged summon spawn should emit update");

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });
    let frame = runtime.flush_effects().expect("summon damage should emit update");

    let summon = runtime.entities.get(EntityIdx(2)).expect("charged summon should exist");
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 10);
    assert_eq!(summon.runtime.hp, 1);
    assert_eq!(summon.runtime.move_state, MoveState { speed_points: 2048 });
    assert_eq!(summon.runtime.policies.damage_share, DamageSharePolicy::None);
    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(frame.updates.updates[0].score, 4);
}

#[test]
fn custom_summon_recast_handler_revives_existing_summon_entity() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summoned_slot = builder
        .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
        .expect("summoned entity slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon-owner",
            "custom.summon_owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3)
                .with_def_res(77, 88)
                .with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        skill_summon_recast_fixture_handler,
        &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("first summon cast should emit spawn update");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(first.updates.updates.len(), 1);
    assert_eq!(first.updates.updates[0].message, "出现一个新的[1]");
    assert_eq!(first.updates.updates[0].target, 2);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
        Some(&SlotValue::U64(2))
    );
    let summon = runtime.entities.get(EntityIdx(2)).expect("summon should exist");
    assert_eq!(summon.template.kind, summon_kind);
    assert_eq!(summon.template.skills.skills(), &[recast_skill]);
    assert_eq!(summon.runtime.owner, EntityIdx(0));
    assert_eq!(summon.runtime.root_owner, EntityIdx(0));
    assert_eq!(summon.runtime.defense, 77);
    assert_eq!(summon.runtime.resistance, 88);

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 10,
    });
    runtime.flush_effects().expect("lethal summon damage should emit update");
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));

    let recast = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("summon recast should revive existing entity");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(recast.updates.updates.len(), 1);
    assert_eq!(recast.updates.updates[0].message, "[1][复活]了");
    assert_eq!(recast.updates.updates[0].target, 2);
    let revived = runtime.entities.get(EntityIdx(2)).expect("summon should revive in place");
    assert!(revived.runtime.alive);
    assert_eq!(revived.runtime.hp, 10);
    assert_eq!(revived.template.skills.skills(), &[recast_skill]);
    assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
}

#[test]
fn custom_summon_recast_handler_can_emit_legacy_summon_messages() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summoned_slot = builder
        .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
        .expect("summoned entity slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon-owner",
            "custom.summon_owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3).with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        skill_legacy_summon_recast_fixture_handler,
        &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("legacy summon cast should emit updates");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(first.updates.updates.len(), 2);
    assert_eq!(first.updates.updates[0].message, "[0]使用[血祭]");
    assert_eq!(first.updates.updates[0].score, 60);
    assert_eq!(first.updates.updates[1].message, "召唤出[1]");
    assert_eq!(first.updates.updates[1].target, 2);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
        Some(&SlotValue::U64(2))
    );
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().template.kind, summon_kind);

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 10,
    });
    runtime.flush_effects().expect("lethal summon damage should emit update");
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);

    let recast = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("legacy summon recast should emit updates");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(recast.updates.updates.len(), 2);
    assert_eq!(recast.updates.updates[0].message, "[0]使用[血祭]");
    assert_eq!(recast.updates.updates[0].score, 60);
    assert_eq!(recast.updates.updates[1].message, "召唤出[1]");
    assert_eq!(recast.updates.updates[1].target, 2);
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
}

#[test]
fn summon_recast_from_template_slot_uses_payload_and_revives_existing_entity() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summoned_slot = builder
        .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
        .expect("summoned entity slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "summon-template", "custom.summon.template")
        .expect("summon template slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon-owner",
            "custom.summon_owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(3, "summon-template", summon_kind, 0, 10, 1)
        .with_def_res(11, 22)
        .with_skills([recast_skill])
        .with_speed_points(2048);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3)
                .with_def_res(77, 88)
                .with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload.clone())))
        .expect("summon template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        run_legacy_summon_recast_from_template_slot,
        &[
            ExtensionCapability::ReadTemplateSlots,
            ExtensionCapability::ReadAllies,
            ExtensionCapability::MutateEntitySlots,
        ],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("template-slot summon cast should emit updates");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(first.updates.updates.len(), 2);
    assert_eq!(first.updates.updates[0].message, "[0]使用[血祭]");
    assert_eq!(first.updates.updates[0].score, 60);
    assert_eq!(first.updates.updates[1].message, "召唤出[1]");
    assert_eq!(first.updates.updates[1].target, 2);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
        Some(&SlotValue::U64(2))
    );
    let summon = runtime.entities.get(EntityIdx(2)).expect("summon should spawn");
    assert_eq!(summon.template.name, payload.name);
    assert_eq!(summon.template.kind, summon_kind);
    assert_eq!(summon.template.skills.skills(), &[recast_skill]);
    assert_eq!(summon.runtime.owner, EntityIdx(0));
    assert_eq!(summon.runtime.root_owner, EntityIdx(0));
    assert_eq!(summon.runtime.defense, 77);
    assert_eq!(summon.runtime.resistance, 88);
    assert_eq!(summon.template.move_state, payload.move_state);
    assert_eq!(summon.runtime.move_state, payload.move_state);

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 10,
    });
    runtime.flush_effects().expect("lethal summon damage should emit update");
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);

    let recast = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("template-slot summon recast should revive existing entity");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(recast.updates.updates.len(), 2);
    assert_eq!(recast.updates.updates[0].message, "[0]使用[血祭]");
    assert_eq!(recast.updates.updates[1].message, "召唤出[1]");
    assert_eq!(recast.updates.updates[1].target, 2);
    let revived = runtime.entities.get(EntityIdx(2)).expect("summon should revive in place");
    assert!(revived.runtime.alive);
    assert_eq!(revived.runtime.hp, 10);
    assert_eq!(revived.template.skills.skills(), &[recast_skill]);
    assert_eq!(revived.runtime.move_state, payload.move_state);
}

#[test]
fn configured_summon_recast_handler_uses_non_default_slots() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .reserve_entity_slot("custom", "unused-entity-slot", "custom.summon.unused_entity")
        .expect("unused entity slot should reserve");
    let summoned_slot = builder
        .reserve_entity_slot("custom", "configured-summon-slot", "custom.summon.configured_entity")
        .expect("configured summoned entity slot should reserve");
    builder
        .reserve_template_slot("custom", "unused-template-slot", "custom.summon.unused_template")
        .expect("unused template slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "configured-summon-template", "custom.summon.configured_template")
        .expect("configured summon template slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "configured-summon-recast",
            "custom.configured_summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("configured summon recast skill should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "configured-summon",
            "custom.configured_summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("configured summon kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(3, "configured-summon", summon_kind, 0, 7, 1);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload.clone())))
        .expect("configured summon template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        skill_configured_summon_recast_handler,
        &[
            ExtensionCapability::ReadTemplateSlots,
            ExtensionCapability::ReadAllies,
            ExtensionCapability::MutateEntitySlots,
        ],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("configured summon recast should emit updates");

    assert_eq!(first.updates.updates[0].message, "[0]使用[血祭]");
    assert_eq!(first.updates.updates[1].message, "召唤出[1]");
    assert_eq!(first.updates.updates[1].target, 2);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().slots.get(EntitySlotId(0)), None);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
        Some(&SlotValue::U64(2))
    );
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().template.name, payload.name);

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 7,
    });
    runtime.flush_effects().expect("configured summon damage should flush");
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);

    let recast = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("configured summon recast should revive existing entity");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(recast.updates.updates[1].target, 2);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 7);
}

#[test]
fn push_summon_recast_from_entity_slot_reports_alive_remembered_summon() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summoned_slot = builder
        .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
        .expect("summoned entity slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
            PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 10, 1),
        ],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .slots
        .set(summoned_slot, SlotValue::U64(2))
        .expect("remembered summon slot should write");
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        skill_records_alive_summon_recast_error,
        &[ExtensionCapability::ReadAllies],
    );

    let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.len(), 3);
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
}

#[test]
fn push_summon_recast_from_entity_slot_reports_missing_read_allies_capability() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summoned_slot = builder
        .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
        .expect("summoned entity slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
            PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 10, 1),
        ],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .slots
        .set(summoned_slot, SlotValue::U64(2))
        .expect("remembered summon slot should write");
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        skill_records_missing_recast_read_allies_error,
        &[ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.len(), 3);
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
}

#[test]
fn next_minion_name_from_entity_slot_allocates_from_root_owner() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let minion_skill = builder
        .register_skill_with_hooks(
            "custom",
            "minion-name",
            "custom.minion_name",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("minion name skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([minion_skill])],
        registry,
    ));
    runtime.set_skill_handler_with_capabilities(
        minion_skill,
        skill_records_next_minion_name,
        &[ExtensionCapability::MutateEntitySlots],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("first minion name should emit update");
    let second = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("second minion name should emit update");

    assert_eq!(first.updates.updates[0].message, "owner?0");
    assert_eq!(second.updates.updates[0].message, "owner?1");
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(2))
    );
}

#[test]
fn next_minion_name_from_entity_slot_uses_root_owner_for_child_minions() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let minion_skill = builder
        .register_skill_with_hooks(
            "custom",
            "minion-name",
            "custom.minion_name",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("minion name skill should register");
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "owner?shadow", minion_kind, 0, 5, 1).with_skills([minion_skill]),
    });
    runtime.flush_effects().expect("child minion spawn should emit update");
    runtime.set_skill_handler_with_capabilities(
        minion_skill,
        skill_records_next_minion_name,
        &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(2), ProcMask::PRE_ACTION)
        .expect("first child minion name should emit update");
    let second = runtime
        .run_skill_hooks(EntityIdx(2), ProcMask::PRE_ACTION)
        .expect("second child minion name should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.root_owner, EntityIdx(0));
    assert_eq!(first.updates.updates[0].message, "owner?0");
    assert_eq!(second.updates.updates[0].message, "owner?1");
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(2))
    );
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().slots.get(counter_slot), None);
}

#[test]
fn next_minion_name_from_entity_slot_requires_root_owner_read_capability() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let minion_skill = builder
        .register_skill_with_hooks(
            "custom",
            "minion-name",
            "custom.minion_name",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("minion name skill should register");
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "owner?shadow", minion_kind, 0, 5, 1).with_skills([minion_skill]),
    });
    runtime.flush_effects().expect("child minion spawn should emit update");
    runtime.set_skill_handler_with_capabilities(
        minion_skill,
        skill_records_missing_minion_name_read_allies_error,
        &[ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime.run_skill_hooks(EntityIdx(2), ProcMask::PRE_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot), None);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().slots.get(counter_slot), None);
}

#[test]
fn push_minion_from_template_with_allocated_name_sets_name_and_spawns() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let minion_skill = builder
        .register_skill_with_hooks(
            "custom",
            "minion-spawn",
            "custom.minion_spawn",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("minion spawn skill should register");
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([minion_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.set_skill_handler_with_capabilities(
        minion_skill,
        skill_pushes_named_minion_spawn,
        &[ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("named minion spawn should emit update");

    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].message, "召唤出[1]");
    assert_eq!(frame.updates.updates[0].caster, 0);
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    let minion = runtime.entities.get(EntityIdx(2)).expect("minion should spawn");
    assert_eq!(minion.template.name, "owner?0");
    assert_eq!(minion.template.kind, minion_kind);
    assert_eq!(minion.runtime.owner, EntityIdx(0));
    assert_eq!(minion.runtime.root_owner, EntityIdx(0));
}

#[test]
fn push_minion_from_template_slot_with_allocated_name_reads_template_and_spawns() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "shadow-template", "custom.minion.shadow_template")
        .expect("shadow template slot should reserve");
    let minion_skill = builder
        .register_skill_with_hooks(
            "custom",
            "minion-template-spawn",
            "custom.minion_template_spawn",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("minion template spawn skill should register");
    let inherited_skill = builder
        .register_skill(
            "custom",
            "possess",
            "custom.minion.possess",
            TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .expect("inherited minion skill should register");
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "shadow",
            "custom.minion.shadow",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("shadow minion kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(3, "placeholder-shadow", minion_kind, 0, 5, 1)
        .with_def_res(2, 3)
        .with_skills([inherited_skill])
        .with_speed_points(-2048);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([minion_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload.clone())))
        .expect("shadow template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        minion_skill,
        skill_pushes_named_minion_from_template_slot,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("named minion template spawn should emit update");

    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].message, "召唤出[1]");
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    let minion = runtime.entities.get(EntityIdx(2)).expect("template minion should spawn");
    assert_eq!(minion.template.name, "owner?0");
    assert_eq!(minion.template.kind, minion_kind);
    assert_eq!(minion.template.max_hp, payload.max_hp);
    assert_eq!(minion.template.defense, payload.defense);
    assert_eq!(minion.template.resistance, payload.resistance);
    assert_eq!(minion.template.skills.skills(), &[inherited_skill]);
    assert_eq!(minion.runtime.owner, EntityIdx(0));
    assert_eq!(minion.runtime.root_owner, EntityIdx(0));
    assert_eq!(minion.template.move_state, payload.move_state);
    assert_eq!(minion.runtime.move_state, payload.move_state);
}

#[test]
fn shadow_style_minion_template_handler_emits_legacy_replay_sequence() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "shadow-template", "custom.minion.shadow_template")
        .expect("shadow template slot should reserve");
    let shadow_skill = builder
        .register_skill_with_hooks(
            "custom",
            "shadow",
            "custom.minion.shadow_skill",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("shadow skill should register");
    let possess_skill = builder
        .register_skill(
            "custom",
            "possess",
            "custom.minion.possess",
            TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .expect("possess skill should register");
    let shadow_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "shadow",
            "custom.minion.shadow",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("shadow minion kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(3, "owner?shadow", shadow_kind, 0, 5, 1)
        .with_skills([possess_skill])
        .with_speed_points(-2048);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([shadow_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload)))
        .expect("shadow template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        shadow_skill,
        run_shadow_minion_from_template_slot,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("shadow-style minion spawn should emit legacy updates");

    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[幻术]");
    assert_eq!(frame.updates.updates[0].caster, 0);
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[0].score, 60);
    assert_eq!(frame.updates.updates[1].message, "召唤出[1]");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 2);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    let shadow = runtime.entities.get(EntityIdx(2)).expect("shadow minion should spawn");
    assert_eq!(shadow.template.name, "owner?0");
    assert_eq!(shadow.template.kind, shadow_kind);
    assert_eq!(shadow.template.skills.skills(), &[possess_skill]);
    assert_eq!(shadow.runtime.owner, EntityIdx(0));
    assert_eq!(shadow.runtime.root_owner, EntityIdx(0));
    assert_eq!(shadow.runtime.move_state, MoveState { speed_points: -2048 });
}

#[test]
fn possess_skill_berserks_target_and_removes_shadow_caster() {
    let mut builder = ExtensionRegistryBuilder::default();
    let possess_skill = builder
        .register_skill_with_hooks(
            "custom",
            "possess",
            "custom.minion.possess",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("possess skill should register");
    let shadow_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "shadow",
            "custom.minion.shadow",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("shadow minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3),
            PlayerTemplate::with_kind(2, "owner?0", shadow_kind, 0, 5, 1).with_skills([possess_skill]),
            PlayerTemplate::new(3, "target", 1, 20, 1),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.owner = EntityIdx(0);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.root_owner = EntityIdx(0);
    runtime.set_skill_handler(possess_skill, run_possess_skill);

    let plan = runtime
        .scheduler
        .skill_hook_plan(&runtime.entities, &runtime.registry, EntityIdx(1), ProcMask::PRE_ACTION);
    let mut updates = RunUpdates::new();
    runtime.drain_skill_hook_plan_with_selected_target_into(&plan, &mut updates, Some(EntityIdx(2)));
    let frame = RuntimeFrame { updates };

    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[附体]");
    assert_eq!(frame.updates.updates[0].caster, 1);
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(frame.updates.updates[1].message, "[1]进入[狂暴]状态");
    assert_eq!(frame.updates.updates[1].caster, 1);
    assert_eq!(frame.updates.updates[1].target, 2);
    assert_eq!(frame.updates.updates[2].message, "[1]消失了");
    assert_eq!(frame.updates.updates[2].caster, 1);
    assert_eq!(frame.updates.updates[2].target, 1);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(2))
            .unwrap()
            .states
            .entry(10)
            .map(|entry| entry.payload.clone()),
        Some(StatePayload::Berserk { step: 4 })
    );
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 0);
    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(2)]);
}

#[test]
fn possess_skill_extends_existing_berserk_state() {
    let mut builder = ExtensionRegistryBuilder::default();
    let possess_skill = builder
        .register_skill_with_hooks(
            "custom",
            "possess",
            "custom.minion.possess",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("possess skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "shadow", 0, 5, 1).with_skills([possess_skill]),
            PlayerTemplate::new(2, "target", 1, 20, 1),
        ],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::berserk(10, 2));
    runtime.set_skill_handler(possess_skill, run_possess_skill);

    let plan = runtime
        .scheduler
        .skill_hook_plan(&runtime.entities, &runtime.registry, EntityIdx(0), ProcMask::PRE_ACTION);
    let mut updates = RunUpdates::new();
    runtime.drain_skill_hook_plan_with_selected_target_into(&plan, &mut updates, Some(EntityIdx(1)));

    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(10)
            .map(|entry| entry.payload.clone()),
        Some(StatePayload::Berserk { step: 6 })
    );
}

#[test]
fn zombie_style_minion_template_handler_emits_legacy_replay_sequence() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "zombie-template", "custom.minion.zombie_template")
        .expect("zombie template slot should reserve");
    let zombie_skill = builder
        .register_skill_with_hooks(
            "custom",
            "zombie",
            "custom.minion.zombie_skill",
            ProcMask::KILL,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("zombie skill should register");
    let zombie_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "zombie",
            "custom.minion.zombie",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("zombie minion kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(3, "owner?zombie", zombie_kind, 0, 4, 1).with_speed_points(1020);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([zombie_skill]),
            PlayerTemplate::new(2, "victim", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload)))
        .expect("zombie template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        zombie_skill,
        run_zombie_minion_from_template_slot,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::KILL)
        .expect("zombie-style minion spawn should emit legacy updates");

    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "\n");
    assert_eq!(frame.updates.updates[1].message, "[0][召唤亡灵]");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 1);
    assert_eq!(frame.updates.updates[1].score, 60);
    assert_eq!(frame.updates.updates[1].delay0, 1500);
    assert_eq!(frame.updates.updates[2].message, "[2]变成了[1]");
    assert_eq!(frame.updates.updates[2].caster, 0);
    assert_eq!(frame.updates.updates[2].target, 2);
    assert_eq!(frame.updates.updates[2].targets.as_slice(), &[1]);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    let zombie = runtime.entities.get(EntityIdx(2)).expect("zombie minion should spawn");
    assert_eq!(zombie.template.name, "owner?0");
    assert_eq!(zombie.template.kind, zombie_kind);
    assert_eq!(zombie.runtime.owner, EntityIdx(0));
    assert_eq!(zombie.runtime.root_owner, EntityIdx(0));
    assert_eq!(zombie.runtime.move_state, MoveState { speed_points: 1020 });
}

#[test]
fn zombie_style_minion_handler_uses_lethal_damage_killed_target() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "zombie-template", "custom.minion.zombie_template")
        .expect("zombie template slot should reserve");
    let zombie_skill = builder
        .register_skill_with_hooks(
            "custom",
            "zombie",
            "custom.minion.zombie_skill",
            ProcMask::KILL,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("zombie skill should register");
    let zombie_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "zombie",
            "custom.minion.zombie",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("zombie minion kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(4, "owner?zombie", zombie_kind, 0, 4, 1);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([zombie_skill]),
            PlayerTemplate::new(2, "first-target", 1, 20, 1),
            PlayerTemplate::new(3, "killed-target", 1, 3, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload)))
        .expect("zombie template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        zombie_skill,
        run_zombie_minion_from_template_slot,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(0),
        target: EntityIdx(2),
        amount: 3,
    });

    let frame = runtime.flush_effects().expect("lethal damage should drive zombie-style minion spawn");

    assert_eq!(frame.updates.updates.len(), 4);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(frame.updates.updates[1].message, "\n");
    assert_eq!(frame.updates.updates[2].message, "[0][召唤亡灵]");
    assert_eq!(frame.updates.updates[2].target, 2);
    assert_eq!(frame.updates.updates[3].message, "[2]变成了[1]");
    assert_eq!(frame.updates.updates[3].target, 3);
    assert_eq!(frame.updates.updates[3].targets.as_slice(), &[2]);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    let zombie = runtime.entities.get(EntityIdx(3)).expect("zombie minion should spawn");
    assert_eq!(zombie.template.name, "owner?0");
    assert_eq!(zombie.template.kind, zombie_kind);
    assert_eq!(zombie.runtime.owner, EntityIdx(0));
    assert_eq!(zombie.runtime.root_owner, EntityIdx(0));
    assert!(!runtime.world.flat_alive().contains(&EntityIdx(2)));
    assert!(runtime.world.flat_alive().contains(&EntityIdx(3)));
}

#[test]
fn configured_minion_handlers_use_non_default_slots_and_targets() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .reserve_entity_slot("custom", "unused-counter", "custom.minion.unused_counter")
        .expect("unused counter slot should reserve");
    let counter_slot = builder
        .reserve_entity_slot("custom", "configured-counter", "custom.minion.configured_counter")
        .expect("configured counter slot should reserve");
    builder
        .reserve_template_slot("custom", "unused-template", "custom.minion.unused_template")
        .expect("unused template slot should reserve");
    let shadow_template_slot = builder
        .reserve_template_slot(
            "custom",
            "configured-shadow-template",
            "custom.minion.configured_shadow_template",
        )
        .expect("configured shadow template slot should reserve");
    let zombie_template_slot = builder
        .reserve_template_slot(
            "custom",
            "configured-zombie-template",
            "custom.minion.configured_zombie_template",
        )
        .expect("configured zombie template slot should reserve");
    let shadow_skill = builder
        .register_skill_with_hooks(
            "custom",
            "configured-shadow",
            "custom.configured_shadow",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("configured shadow skill should register");
    let zombie_skill = builder
        .register_skill_with_hooks(
            "custom",
            "configured-zombie",
            "custom.configured_zombie",
            ProcMask::KILL,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("configured zombie skill should register");
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "configured-minion",
            "custom.configured_minion",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("configured minion kind should register");
    let registry = builder.build();
    let shadow_payload = PlayerTemplate::with_kind(3, "shadow-template", minion_kind, 0, 5, 1);
    let zombie_payload = PlayerTemplate::with_kind(4, "zombie-template", minion_kind, 0, 6, 1);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([shadow_skill, zombie_skill]),
            PlayerTemplate::new(2, "victim-a", 1, 10, 1),
            PlayerTemplate::new(3, "victim-b", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(shadow_template_slot, SlotValue::PlayerTemplate(Box::new(shadow_payload)))
        .expect("configured shadow template slot should write");
    template
        .slots
        .set(zombie_template_slot, SlotValue::PlayerTemplate(Box::new(zombie_payload)))
        .expect("configured zombie template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        shadow_skill,
        skill_configured_shadow_minion_handler,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );
    runtime.set_skill_handler_with_capabilities(
        zombie_skill,
        skill_configured_zombie_minion_handler,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );

    let shadow_frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("configured shadow minion should emit updates");

    assert_eq!(shadow_frame.updates.updates[0].message, "[0]使用[幻术]");
    assert_eq!(shadow_frame.updates.updates[1].target, 3);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().slots.get(EntitySlotId(0)), None);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().template.name, "owner?0");

    let zombie_frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::KILL)
        .expect("configured zombie minion should emit updates");

    assert_eq!(zombie_frame.updates.updates[1].message, "[0][召唤亡灵]");
    assert_eq!(zombie_frame.updates.updates[1].target, 2);
    assert_eq!(zombie_frame.updates.updates[2].message, "[2]变成了[1]");
    assert_eq!(zombie_frame.updates.updates[2].target, 4);
    assert_eq!(zombie_frame.updates.updates[2].targets.as_slice(), &[2]);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(2))
    );
    assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().template.name, "owner?1");
}

#[test]
fn minion_display_index_for_entity_matches_legacy_name_suffix() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3),
            PlayerTemplate::with_kind(2, "owner?0", minion_kind, 0, 5, 1),
            PlayerTemplate::with_kind(3, "owner?12", minion_kind, 0, 5, 1),
            PlayerTemplate::with_kind(4, "owner?shadow", minion_kind, 0, 5, 1),
            PlayerTemplate::with_kind(5, "shadow", minion_kind, 0, 5, 1),
        ],
        registry,
    ));

    assert_eq!(minion_display_index_for_entity(None), 0);
    assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(0))), 0);
    assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(1))), 1);
    assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(2))), 13);
    assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(3))), 1);
    assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(4))), 1);
}

#[test]
fn custom_minion_heal_fixture_does_not_share_with_owner_or_summons() {
    let mut builder = ExtensionRegistryBuilder::default();
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion-owner",
            "custom.minion_owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION | PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3),
            PlayerTemplate::new(2, "healer", 0, 10, 1),
            PlayerTemplate::new(3, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "summon-a", summon_kind, 0, 10, 1),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(5, "summon-b", summon_kind, 0, 10, 1),
    });
    runtime.flush_effects().expect("minion spawns should emit updates");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(2),
        target: EntityIdx(0),
        amount: 4,
    });
    let shared_damage = runtime.flush_effects().expect("owner damage should share to minions");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 16);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 6);
    assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().runtime.hp, 6);
    assert_eq!(shared_damage.updates.updates.len(), 3);
    assert_eq!(shared_damage.updates.updates[0].target, 0);
    assert_eq!(shared_damage.updates.updates[1].target, 3);
    assert_eq!(shared_damage.updates.updates[2].target, 4);

    runtime.effects.push(QueuedEffect::Heal {
        caster: EntityIdx(1),
        target: EntityIdx(3),
        amount: 3,
    });
    let minion_heal = runtime.flush_effects().expect("minion heal should emit one update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 16);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 9);
    assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().runtime.hp, 6);
    assert_eq!(minion_heal.updates.updates.len(), 1);
    assert_eq!(minion_heal.updates.updates[0].message, "[1]回复体力[2]点");
    assert_eq!(minion_heal.updates.updates[0].target, 3);
    assert_eq!(minion_heal.updates.updates[0].score, 3);
}

#[test]
fn custom_minion_owner_death_removes_linked_minions_in_entity_order() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION | PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
    });
    runtime.flush_effects().expect("minion spawns should emit updates");

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(0),
        amount: 10,
    });
    let frame = runtime.flush_effects().expect("owner death should cleanup linked minions");

    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert!(!runtime.entities.get(EntityIdx(3)).unwrap().runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
    assert_eq!(runtime.world.round_order(), &[EntityIdx(1)]);
    assert_eq!(runtime.world.team_alive(0), Some([].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(1)]);
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert_eq!(frame.updates.updates.len(), 5);
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(
        frame.updates.updates[1].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[2].target, 2);
    assert_eq!(frame.updates.updates[2].message, "[1]消失了");
    assert_eq!(frame.updates.updates[2].score, 50);
    assert_eq!(
        frame.updates.updates[3].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[4].target, 3);
    assert_eq!(frame.updates.updates[4].message, "[1]消失了");
    assert_eq!(frame.updates.updates[4].score, 50);
}

#[test]
fn custom_minion_owner_remove_cleans_linked_minions_in_entity_order() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION | PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
    });
    runtime.flush_effects().expect("minion spawns should emit updates");

    runtime.effects.push(QueuedEffect::Remove {
        caster: EntityIdx(1),
        target: EntityIdx(0),
    });
    let frame = runtime.flush_effects().expect("owner remove should cleanup linked minions");

    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert!(!runtime.entities.get(EntityIdx(3)).unwrap().runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
    assert_eq!(runtime.world.round_order(), &[EntityIdx(1)]);
    assert_eq!(runtime.world.team_alive(0), Some([].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(1)]);
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert_eq!(frame.updates.updates.len(), 5);
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[0].message, "[1]消失了");
    assert_eq!(
        frame.updates.updates[1].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[2].target, 2);
    assert_eq!(frame.updates.updates[2].message, "[1]消失了");
    assert_eq!(frame.updates.updates[2].score, 50);
    assert_eq!(
        frame.updates.updates[3].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[4].target, 3);
    assert_eq!(frame.updates.updates[4].message, "[1]消失了");
    assert_eq!(frame.updates.updates[4].score, 50);
}

#[test]
fn custom_runner_fixture_matches_strict_diff_golden() {
    let mut builder = ExtensionRegistryBuilder::default();
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "runner-owner",
            "custom.runner_owner",
            PlayerKindFlags::BED2,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("runner owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "runner-summon",
            "custom.runner_summon",
            PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("runner summon kind should register");
    let hp_marker = builder
        .reserve_entity_slot("custom", "hp-marker", "custom.hp_marker")
        .expect("hp marker slot should reserve");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3).with_def_res(77, 88),
            PlayerTemplate::new(2, "healer", 0, 10, 1),
            PlayerTemplate::new(3, "enemy", 1, 10, 1),
        ],
        registry,
    ));

    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "summon", summon_kind, 0, 10, 1).with_def_res(11, 22),
    });
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(2),
        target: EntityIdx(0),
        amount: 4,
    });
    runtime.effects.push(QueuedEffect::Heal {
        caster: EntityIdx(1),
        target: EntityIdx(3),
        amount: 2,
    });
    runtime.effects.push(QueuedEffect::Replay {
        caster: EntityIdx(0),
        target: EntityIdx(0),
        message: "[0]还剩[2]点血".to_owned(),
        score: 87,
    });

    let frame = runtime.flush_effects().expect("custom runner fixture should emit updates");
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .slots
        .set(hp_marker, SlotValue::Bool(true))
        .expect("hp marker slot should write");
    let outcome = RoundOutcome {
        action: None,
        frame: Some(frame),
        winner_team: runtime.world.sync_winner(&runtime.entities),
    };
    let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
    let expected = NormalizedOutcome {
        winner_team: None,
        round: 0,
        total_score: 97,
        rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
        entity_ids: vec![1, 2, 3, 4],
        teams: vec![0, 0, 1, 0],
        hp: vec![16, 10, 10, 8],
        magic_point: vec![0, 0, 0, 0],
        defense: vec![77, 0, 0, 77],
        resistance: vec![88, 0, 0, 88],
        alive: vec![true, true, true, true],
        round_order: vec![0, 1, 2, 3],
        flat_alive: vec![0, 1, 3, 2],
        team_alive: vec![vec![0, 1, 3], vec![2]],
        alive_group_count: 2,
        actions: Vec::new(),
        frames: vec![
            NormalizedUpdateFrame {
                message: "出现一个新的[1]".to_owned(),
                caster: 0,
                target: 3,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 2,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 4,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 2,
                target: 3,
                targets: Vec::new(),
                param: None,
                score: 4,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "[1]回复体力[2]点".to_owned(),
                caster: 1,
                target: 3,
                targets: Vec::new(),
                param: None,
                score: 2,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "[0]还剩[2]点血".to_owned(),
                caster: 0,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 87,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
        ],
    };

    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(hp_marker),
        Some(&SlotValue::Bool(true))
    );
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().template.defense, 77);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().template.resistance, 88);
    assert_eq!(strict_diff(&expected, &actual), Ok(()));
}

#[test]
fn custom_runner_minion_owner_death_matches_strict_diff_golden() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "runner-linked-minion",
            "custom.runner_linked_minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION | PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("runner minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));

    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
    });
    runtime.flush_effects().expect("minion spawns should emit updates");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(0),
        amount: 10,
    });

    let frame = runtime.flush_effects().expect("owner death should cleanup linked minions");
    let outcome = RoundOutcome {
        action: None,
        frame: Some(frame),
        winner_team: runtime.world.sync_winner(&runtime.entities),
    };
    let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
    let expected = NormalizedOutcome {
        winner_team: Some(1),
        round: 0,
        total_score: 110,
        rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
        entity_ids: vec![1, 2, 3, 4],
        teams: vec![0, 1, 0, 0],
        hp: vec![0, 10, 0, 0],
        magic_point: vec![0, 0, 0, 0],
        defense: vec![0, 0, 0, 0],
        resistance: vec![0, 0, 0, 0],
        alive: vec![false, true, false, false],
        round_order: vec![1],
        flat_alive: vec![1],
        team_alive: vec![Vec::new(), vec![1]],
        alive_group_count: 1,
        actions: Vec::new(),
        frames: vec![
            NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 1,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 10,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "\n".to_owned(),
                caster: 0,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: 0,
                delay1: 0,
                update_type: crate::engine::update::UpdateType::NextLine,
            },
            NormalizedUpdateFrame {
                message: "[1]消失了".to_owned(),
                caster: 0,
                target: 2,
                targets: Vec::new(),
                param: None,
                score: 50,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "\n".to_owned(),
                caster: 0,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: 0,
                delay1: 0,
                update_type: crate::engine::update::UpdateType::NextLine,
            },
            NormalizedUpdateFrame {
                message: "[1]消失了".to_owned(),
                caster: 0,
                target: 3,
                targets: Vec::new(),
                param: None,
                score: 50,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
        ],
    };

    assert_eq!(strict_diff(&expected, &actual), Ok(()));
}

#[test]
fn custom_runner_merge_matches_strict_diff_golden() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill_a = builder
        .register_skill("custom", "runner-a", "custom.runner_a", TargetPolicy::Enemy, SkillPriority(0))
        .expect("runner skill should register");
    let skill_b = builder
        .register_skill("custom", "runner-b", "custom.runner_b", TargetPolicy::Enemy, SkillPriority(1))
        .expect("runner skill should register");
    let skill_c = builder
        .register_skill("custom", "runner-c", "custom.runner_c", TargetPolicy::Enemy, SkillPriority(2))
        .expect("runner skill should register");
    let merge_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "runner-merge",
            "custom.runner_merge",
            PlayerKindFlags::NONE,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: false,
            },
        )
        .expect("runner merge kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "merge-owner", merge_kind, 0, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_a, 1)])),
            PlayerTemplate::new(2, "merge-target", 1, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_b, 2), (skill_c, 3)])),
        ],
        registry,
    ));

    runtime.effects.push(QueuedEffect::Merge {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    let frame = runtime.flush_effects().expect("runner merge should emit updates");
    let outcome = RoundOutcome {
        action: None,
        frame: Some(frame),
        winner_team: runtime.world.sync_winner(&runtime.entities),
    };
    let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
    let expected = NormalizedOutcome {
        winner_team: None,
        round: 0,
        total_score: 60,
        rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
        entity_ids: vec![1, 2],
        teams: vec![0, 1],
        hp: vec![10, 10],
        magic_point: vec![0, 0],
        defense: vec![0, 0],
        resistance: vec![0, 0],
        alive: vec![true, true],
        round_order: vec![0, 1],
        flat_alive: vec![0, 1],
        team_alive: vec![vec![0], vec![1]],
        alive_group_count: 2,
        actions: Vec::new(),
        frames: vec![
            NormalizedUpdateFrame {
                message: "\n".to_owned(),
                caster: 0,
                target: 0,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: 0,
                delay1: 0,
                update_type: crate::engine::update::UpdateType::NextLine,
            },
            NormalizedUpdateFrame {
                message: "[0][吞噬]了[1]".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 60,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
            NormalizedUpdateFrame {
                message: "[0]属性上升".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 0,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            },
        ],
    };

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill_a]);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.levels(), &[2]);
    assert_eq!(strict_diff(&expected, &actual), Ok(()));
}

fn custom_marks_update(context: &mut EffectContext<'_>, effect: &CustomEffect) {
    let CustomEffectPayload::Text(message) = &effect.payload else {
        panic!("custom test effect expects text payload");
    };
    context.add_update(crate::engine::update::RunUpdate::new(
        message.clone(),
        effect.caster.0 as usize,
        effect.target.unwrap().0 as usize,
        0,
    ));
}

fn custom_spawns_nested_damage(context: &mut EffectContext<'_>, effect: &CustomEffect) {
    let CustomEffectPayload::Int(amount) = effect.payload else {
        panic!("custom test effect expects int payload");
    };
    context.push_nested(QueuedEffect::Damage {
        caster: effect.caster,
        target: effect.target.expect("custom test effect needs target"),
        amount,
    });
}

fn custom_spawns_nested_heal(context: &mut EffectContext<'_>, effect: &CustomEffect) {
    let CustomEffectPayload::Int(amount) = effect.payload else {
        panic!("custom test effect expects int payload");
    };
    context.push_nested(QueuedEffect::Heal {
        caster: effect.caster,
        target: effect.target.expect("custom test effect needs target"),
        amount,
    });
}

fn custom_rejects_cross_entity_read(context: &mut EffectContext<'_>, _: &CustomEffect) {
    assert_eq!(
        context.entity(EntityIdx(2)),
        Err(EffectContextError::MissingCapability(ExtensionCapability::ReadEnemies))
    );
    context.add_update(crate::engine::update::RunUpdate::new("read denied", 0, 0, 0));
}

fn custom_reads_cross_entity(context: &mut EffectContext<'_>, _: &CustomEffect) {
    let observed = context.entity(EntityIdx(2)).expect("capability should allow cross-entity read");
    context.add_update(crate::engine::update::RunUpdate::new(observed.template.name.clone(), 0, 2, 0));
}

fn custom_mutates_entity_slot(context: &mut EffectContext<'_>, effect: &CustomEffect) {
    let CustomEffectPayload::Int(slot) = effect.payload else {
        panic!("custom test effect expects entity slot id payload");
    };
    context
        .set_entity_slot(
            effect.target.expect("custom test effect needs target"),
            EntitySlotId(slot as u32),
            SlotValue::Bool(true),
        )
        .expect("capability should allow entity slot mutation");
    context.add_update(crate::engine::update::RunUpdate::new("slot set", 0, 0, 0));
}

fn custom_consumes_rng(context: &mut EffectContext<'_>, effect: &CustomEffect) {
    let CustomEffectPayload::Int(max) = effect.payload else {
        panic!("custom test effect expects rng max payload");
    };
    let value = context.rng_next_i32(max);
    let next_byte = context.rng_next_u8();
    context.add_update(crate::engine::update::RunUpdate::new(
        format!("rng:{value}:{next_byte}"),
        effect.caster.0 as usize,
        effect.target.unwrap().0 as usize,
        value as u32,
    ));
}

fn skill_marks_update(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    context.add_update(crate::engine::update::RunUpdate::new(
        "skill mark",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.skill_id.0,
    ));
}

fn skill_marks_selected_target(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let target = context.selected_target().expect("skill should receive selected target");
    context.add_update(crate::engine::update::RunUpdate::new(
        "selected target",
        entry.owner.0 as usize,
        target.0 as usize,
        target.0,
    ));
}

fn state_marks_charge_boost(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let owner = context.owner().expect("state owner should exist");
    let message = if owner.runtime.charge.active && owner.runtime.at_boost_millionths == 3_000_000 {
        "charge boosted"
    } else {
        "charge inactive"
    };
    context.add_update(crate::engine::update::RunUpdate::new(
        message,
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.legacy_order_key,
    ));
}

fn skill_clears_positive_runtime(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let messages = context
        .clear_owner_positive_runtime_messages()
        .expect("clear-positive owner should exist");
    let owner = context.owner_idx();
    for (priority, message) in messages {
        context.add_update(crate::engine::update::RunUpdate::new(
            message,
            owner.0 as usize,
            owner.0 as usize,
            priority as u32,
        ));
    }
}

fn skill_clears_positive_states(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let messages = context.clear_owner_positive_state_messages().expect("clear-positive owner should exist");
    let owner = context.owner_idx();
    for (priority, message) in messages {
        context.add_update(crate::engine::update::RunUpdate::new(
            message,
            owner.0 as usize,
            owner.0 as usize,
            priority as u32,
        ));
    }
}

fn skill_clears_positive(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let messages = context.clear_owner_positive_messages().expect("clear-positive owner should exist");
    let owner = context.owner_idx();
    for (priority, message) in messages {
        context.add_update(crate::engine::update::RunUpdate::new(
            message,
            owner.0 as usize,
            owner.0 as usize,
            priority as u32,
        ));
    }
}

fn skill_noop(_: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {}

fn skill_pushes_nested_damage(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    context.push_nested(QueuedEffect::Damage {
        caster: context.owner_idx(),
        target: EntityIdx(1),
        amount: 2,
    });
}

fn skill_halves_defend_atp(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let atp = context.defend_atp().expect("pre-defend skill should receive atp");
    context.add_update(crate::engine::update::RunUpdate::new(
        "pre defend skill",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        atp as u32,
    ));
    context.set_defend_atp(atp / 2.0);
}

fn skill_zeroes_defend_atp(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    assert!(context.defend_atp().expect("pre-defend skill should receive atp") > 0.0);
    context.add_update(crate::engine::update::RunUpdate::new(
        "pre defend zero",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.skill_id.0,
    ));
    context.set_defend_atp(0.0);
}

fn skill_marks_defend_replay(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let caster = context.defend_caster().expect("post-defend skill should receive incoming caster");
    let target = context.defend_target().expect("post-defend skill should receive incoming target");
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0][防御]",
        target.0 as usize,
        caster.0 as usize,
        0,
    ));
}

fn skill_halves_defend_damage(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let damage = context.defend_damage().expect("post-defend skill should receive damage");
    context.add_update(crate::engine::update::RunUpdate::new(
        "post defend skill",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        damage as u32,
    ));
    context.set_defend_damage(damage / 2);
}

fn skill_bed2_template_slot_summon_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    push_summon_from_template_slot(context, TemplateSlotId(0)).expect("bed2 summon handler should read template slot payload");
}

fn skill_bed2_template_slot_legacy_summon_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    push_summon_from_template_slot_with_message(context, TemplateSlotId(0), "召唤出[1]")
        .expect("bed2 summon handler should read template slot payload");
}

fn skill_records_missing_template_slot_error(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    assert_eq!(
        push_summon_from_template_slot(context, TemplateSlotId(0)),
        Err(RuntimeV2SummonHandlerError::MissingTemplateSlot(TemplateSlotId(0)))
    );
    context.add_update(crate::engine::update::RunUpdate::new(
        "missing summon template",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        0,
    ));
}

fn skill_consumes_rng(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let value = context.rng_next_i32(10);
    let next_byte = context.rng_next_u8();
    context.add_update(crate::engine::update::RunUpdate::new(
        format!("skill-rng:{value}:{next_byte}"),
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        value as u32,
    ));
}

fn skill_summon_recast_fixture_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
        .with_def_res(11, 22)
        .with_skills([SkillId(0)]);
    push_summon_recast_from_entity_slot(context, EntitySlotId(0), summon_template, 10)
        .expect("summon recast fixture should spawn or revive summon");
}

fn skill_legacy_summon_recast_fixture_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0]使用[血祭]",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        60,
    ));
    let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
        .with_def_res(11, 22)
        .with_skills([SkillId(0)]);
    push_summon_recast_from_entity_slot_with_messages(context, EntitySlotId(0), summon_template, 10, "召唤出[1]", "召唤出[1]")
        .expect("legacy summon recast fixture should spawn or revive summon");
}

fn skill_configured_summon_recast_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_legacy_summon_recast_from_template_slot_with_config(context, EntitySlotId(1), TemplateSlotId(1), 7);
}

fn skill_records_alive_summon_recast_error(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
        .with_def_res(11, 22)
        .with_skills([SkillId(0)]);
    assert_eq!(
        push_summon_recast_from_entity_slot(context, EntitySlotId(0), summon_template, 10),
        Err(RuntimeV2SummonHandlerError::RememberedSummonAlive(EntityIdx(2)))
    );
}

fn skill_records_missing_recast_read_allies_error(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
        .with_def_res(11, 22)
        .with_skills([SkillId(0)]);
    assert_eq!(
        push_summon_recast_from_entity_slot(context, EntitySlotId(0), summon_template, 10),
        Err(RuntimeV2SummonHandlerError::Context(EffectContextError::MissingCapability(
            ExtensionCapability::ReadAllies
        )))
    );
}

fn skill_records_next_minion_name(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let name = next_minion_name_from_entity_slot(context, EntitySlotId(0)).expect("minion name helper should allocate a name");
    context.add_update(crate::engine::update::RunUpdate::new(
        name,
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        0,
    ));
}

fn skill_records_missing_minion_name_read_allies_error(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    assert_eq!(
        next_minion_name_from_entity_slot(context, EntitySlotId(0)),
        Err(RuntimeV2MinionHandlerError::Context(EffectContextError::MissingCapability(
            ExtensionCapability::ReadAllies
        )))
    );
}

fn skill_pushes_named_minion_spawn(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let minion_template = PlayerTemplate::with_kind(3, "placeholder", PlayerKindId(0), 0, 5, 1);
    assert_eq!(
        push_minion_from_template_with_allocated_name(context, EntitySlotId(0), minion_template, "召唤出[1]"),
        Ok(EntityIdx(2))
    );
}

fn skill_pushes_named_minion_from_template_slot(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    assert_eq!(
        push_minion_from_template_slot_with_allocated_name(context, EntitySlotId(0), TemplateSlotId(0), "召唤出[1]"),
        Ok(EntityIdx(2))
    );
}

fn skill_configured_shadow_minion_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_shadow_minion_from_template_slot_with_config(context, EntitySlotId(1), TemplateSlotId(1));
}

fn skill_configured_zombie_minion_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_zombie_minion_from_template_slot_with_config(context, EntitySlotId(1), TemplateSlotId(2), EntityIdx(2));
}

fn state_marks_update(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    context.add_update(crate::engine::update::RunUpdate::new(
        "state mark",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.legacy_order_key,
    ));
}

fn state_pushes_nested_heal(context: &mut StateContext<'_>, _: &StateHookPlanEntry) {
    context.push_nested(QueuedEffect::Heal {
        caster: context.owner_idx(),
        target: context.owner_idx(),
        amount: 2,
    });
}

fn state_consumes_rng(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let value = context.rng_next_i32(10);
    let next_byte = context.rng_next_u8();
    context.add_update(crate::engine::update::RunUpdate::new(
        format!("state-rng:{value}:{next_byte}"),
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        value as u32,
    ));
}

fn state_adds_defend_damage(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let damage = context.defend_damage().expect("post-defend state should receive damage");
    context.add_update(crate::engine::update::RunUpdate::new(
        "post defend state",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.legacy_order_key,
    ));
    context.set_defend_damage(damage + 3);
}

fn render_first_message_replay(frame: &RuntimeFrame) -> Option<RenderedReplay> {
    Some(RenderedReplay::new(
        ReplayRendererId(0),
        frame.updates.updates.first()?.message.to_string(),
    ))
}

fn render_update_count_replay(frame: &RuntimeFrame) -> Option<RenderedReplay> {
    Some(RenderedReplay::new(
        ReplayRendererId(1),
        frame.updates.updates.len().to_string(),
    ))
}

fn render_first_message_show(frame: &RuntimeFrame) -> Option<RenderedShow> {
    Some(RenderedShow::new(
        ShowRendererId(0),
        frame.updates.updates.first()?.message.to_string(),
    ))
}

fn render_hp_marker_bar_show(frame: &RuntimeFrame) -> Option<RenderedShow> {
    let hp_report = frame.updates.updates.iter().find(|update| update.message == "[0]还剩[2]点血")?;
    Some(RenderedShow::new(
        ShowRendererId(0),
        format!(
            "hp-bar:actor={}:value={}:text={}",
            hp_report.caster,
            hp_report.param.unwrap_or(hp_report.score),
            hp_report.msg()
        ),
    ))
}

fn mixed_raw_runner_for_plain_fixture(raw_input: &str) -> (RuntimeV2Runner, crate::Runner) {
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
    let runner = prepared_init_tests::runtime_v2_runner_from_raw(raw_input, |raw_groups| {
        RuntimeV2Runner::from_mixed_roster(raw_groups, registry, bed2, summon)
    });
    let legacy =
        crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("plain raw fixture should construct legacy runner");
    (runner, legacy)
}

fn assert_runtime_rng_matches_legacy(runtime: &CombatRuntime, legacy: &crate::Runner) {
    assert_eq!(runtime.rng.i, legacy.randomer.i);
    assert_eq!(runtime.rng.j, legacy.randomer.j);
    assert_eq!(runtime.rng.main_val, legacy.randomer.main_val);
}

fn assert_runtime_world_matches_legacy_raw_world(runtime: &CombatRuntime, legacy_world: &crate::engine::world_state::WorldState) {
    assert_eq!(
        runtime.world.round_order(),
        legacy_entity_order(&legacy_world.players).as_slice()
    );
    assert_eq!(
        runtime.world.flat_alive(),
        legacy_entity_order(&legacy_world.flat_alive).as_slice()
    );
    assert_eq!(runtime.world.alive_group_count(), legacy_world.alive_group_count());
    for team_idx in 0..legacy_world.groups.len() {
        assert_eq!(
            runtime.world.team_alive(team_idx),
            Some(legacy_entity_order(legacy_world.team_alive(team_idx).unwrap_or_default()).as_slice())
        );
    }
    for (team_idx, group) in legacy_world.groups.iter().enumerate() {
        for plr in group {
            let entity = runtime
                .entities
                .get(EntityIdx(
                    (*plr).try_into().expect("legacy fixture player id should fit EntityIdx"),
                ))
                .expect("legacy raw world player should exist in runtime_v2");
            assert_eq!(entity.runtime.team, team_idx);
            assert_eq!(entity.template.team, team_idx);
        }
    }
}

fn legacy_entity_order(plrs: &[crate::player::PlrId]) -> Vec<EntityIdx> {
    plrs.iter()
        .copied()
        .map(|plr| EntityIdx(plr.try_into().expect("legacy fixture player id should fit EntityIdx")))
        .collect()
}

#[test]
fn run_skill_hooks_dispatches_registered_skill_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let marker = builder
        .register_skill_with_hooks(
            "custom",
            "marker",
            "custom.marker",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([marker])],
        registry,
    ));
    runtime.set_skill_handler(marker, skill_marks_update);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("skill handler should emit update");

    assert_eq!(frame.updates.updates[0].message, "skill mark");
    assert_eq!(frame.updates.updates[0].score, marker.0);
}

#[test]
fn run_skill_hooks_exposes_controlled_rng_to_skill_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill_with_hooks(
            "custom",
            "rng-skill",
            "custom.rng_skill",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill])],
        registry,
    ));
    runtime.set_skill_handler(skill, skill_consumes_rng);
    let mut expected_rng = RC4::default();
    let expected_value = expected_rng.next_i32(10);
    let expected_byte = expected_rng.next_u8();

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("skill rng handler should emit update");

    assert_eq!(
        frame.updates.updates[0].message,
        format!("skill-rng:{expected_value}:{expected_byte}")
    );
    assert_eq!(frame.updates.updates[0].score, expected_value as u32);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
}

#[test]
fn run_skill_hooks_flushes_nested_effects() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill_with_hooks(
            "custom",
            "damage",
            "custom.damage",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(skill, skill_pushes_nested_damage);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("nested damage should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 8);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[0].score, 2);
}

#[test]
fn run_skill_hooks_disperse_without_selected_target_noops() {
    let mut builder = ExtensionRegistryBuilder::default();
    let disperse = builder
        .register_skill_with_hooks(
            "core",
            "disperse",
            "core.disperse",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("disperse skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([disperse]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(disperse, run_disperse_skill);

    let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
}

#[test]
fn run_minimal_round_dispatches_pre_action_skill_before_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let marker = builder
        .register_skill_with_hooks(
            "custom",
            "marker",
            "custom.marker",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([marker]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(marker, skill_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("skill plus attack should emit update");

    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "skill mark");
    assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
}

#[test]
fn run_minimal_round_flushes_pre_action_skill_effect_before_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill_with_hooks(
            "custom",
            "damage",
            "custom.damage",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(skill, skill_pushes_nested_damage);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("skill damage plus attack should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 5);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].score, 2);
    assert_eq!(frame.updates.updates[1].score, 3);
}

#[test]
fn plain_charge_selects_self_executes_and_ticks_in_late_post_action() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charge = builder
        .register_skill_with_hooks_and_post_action_phase(
            "core",
            "charge",
            BuiltinActiveSkill::Charge.export_name(),
            ProcMask::POST_ACTION,
            TargetPolicy::None,
            SkillPriority(19),
            SkillPostActionPhase::Late,
        )
        .expect("charge skill should register");
    let registry = builder.build();
    let loadout = SkillLoadout::from_skill_levels([(charge, 128)]);
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_skill_loadout(loadout),
            PlayerTemplate::new(2, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(charge, run_charge_post_action_skill);

    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("charge should be selected when probability passes");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Charge);
    assert_eq!(prepared.targets, vec![EntityIdx(0)]);

    let mut updates = RunUpdates::new();
    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(updates.updates[0].message, "[0]开始[蓄力]");
    assert_eq!(owner.runtime.magic_point, 32);
    assert!(owner.runtime.charge.active);
    assert_eq!(owner.runtime.charge.step, 2);
    assert_eq!(owner.runtime.at_boost_millionths, 3_000_000);

    let plan = runtime.scheduler.skill_post_action_hook_plan(
        &runtime.entities,
        &runtime.registry,
        EntityIdx(0),
        SkillPostActionPhase::Late,
    );
    runtime.drain_skill_hook_plan_into(&plan, &mut updates);
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(owner.runtime.charge.active);
    assert_eq!(owner.runtime.charge.step, 1);
}

#[test]
fn plain_reraise_revives_halves_level_and_stops_kill_hooks() {
    let mut builder = ExtensionRegistryBuilder::default();
    let kill = builder
        .register_skill_with_hooks(
            "custom",
            "kill-marker",
            "custom.kill_marker",
            ProcMask::KILL,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("kill marker should register");
    let reraise = builder
        .register_skill_with_hooks(
            "core",
            "reraise",
            DEFAULT_CORE_RERAISE_SKILL_EXPORT,
            ProcMask::DIE,
            TargetPolicy::None,
            SkillPriority(10),
        )
        .expect("reraise skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "killer", 0, 100, 3).with_skills([kill]),
            PlayerTemplate::new(2, "target", 1, 100, 3).with_skill_loadout(SkillLoadout::from_skill_levels([(reraise, 128)])),
        ],
        registry,
    ));
    runtime.set_skill_handler(kill, skill_marks_selected_target);
    runtime.set_skill_handler(reraise, run_reraise_die_skill);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 0;

    let mut updates = RunUpdates::new();
    runtime.drain_plain_lethal_damage_into(EntityIdx(0), EntityIdx(1), &mut updates);

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert!(target.runtime.alive);
    assert!((1..=16).contains(&target.runtime.hp));
    assert_eq!(target.template.skills.level_at(0), Some(64));
    assert!(runtime.world.flat_alive().contains(&EntityIdx(1)));
    assert_eq!(
        updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec!["[1]被击倒了", "[0]使用[护身符]抵挡了一次死亡", "[1]回复体力[2]点"]
    );
    assert!(!updates.updates.iter().any(|update| update.message == "selected target"));
}

#[test]
fn score_disperse_target_matches_legacy_smart_two_team_formula() {
    let runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "caster", 0, 10, 3),
        PlayerTemplate::new(2, "target", 1, 80, 3).with_target_score_stats(77, 120, 2.5),
    ]));
    let mut rng = RC4::default();

    let score = score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(1), true, &mut rng);

    assert_eq!(score, (1.0 / 80.0) * 120.0 * 2.5);
    let expected_rng = RC4::default();
    assert_eq!(rng.i, expected_rng.i);
    assert_eq!(rng.j, expected_rng.j);
    assert_eq!(rng.main_val, expected_rng.main_val);
}

#[test]
fn score_disperse_target_matches_legacy_smart_multi_team_and_minion_formula() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies::default(),
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3),
            PlayerTemplate::with_kind(2, "target", minion_kind, 1, 400, 3).with_target_score_stats(77, 120, 2.5),
            PlayerTemplate::new(3, "team-2", 2, 10, 3),
            PlayerTemplate::new(4, "team-1-ally", 1, 10, 3),
        ],
        registry,
    ));
    let mut rng = RC4::default();

    let score = score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(1), true, &mut rng);

    assert_eq!(score, 300.0 * 2.0 * 2.5 * 2.0);
    let expected_rng = RC4::default();
    assert_eq!(rng.i, expected_rng.i);
    assert_eq!(rng.j, expected_rng.j);
    assert_eq!(rng.main_val, expected_rng.main_val);
}

#[test]
fn score_disperse_target_matches_legacy_random_formula_and_unknown_target() {
    let runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "caster", 0, 10, 3),
        PlayerTemplate::new(2, "target", 1, 80, 3).with_target_score_stats(77, 120, 2.5),
    ]));
    let mut rng = RC4::default();
    let mut expected_rng = RC4::default();
    let expected = expected_rng.rFFFF() as f64 + 2.5;

    assert_eq!(
        score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(1), false, &mut rng),
        expected
    );
    assert_eq!(rng.i, expected_rng.i);
    assert_eq!(rng.j, expected_rng.j);
    assert_eq!(rng.main_val, expected_rng.main_val);
    assert_eq!(
        score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(99), false, &mut rng),
        f64::MIN
    );
}

#[test]
fn run_minimal_round_dispatches_damage_skill_hooks_around_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let pre_damage = builder
        .register_skill_with_hooks(
            "custom",
            "pre-damage",
            "custom.pre_damage",
            ProcMask::PRE_DAMAGE,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("pre-damage skill should register");
    let post_damage = builder
        .register_skill_with_hooks(
            "custom",
            "post-damage",
            "custom.post_damage",
            ProcMask::POST_DAMAGE,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("post-damage skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([pre_damage, post_damage]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(pre_damage, skill_marks_update);
    runtime.set_skill_handler(post_damage, skill_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("damage skill hooks plus attack should emit update");

    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "skill mark");
    assert_eq!(frame.updates.updates[0].score, pre_damage.0);
    assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[2].message, "skill mark");
    assert_eq!(frame.updates.updates[2].score, post_damage.0);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
}

#[test]
fn run_minimal_round_dispatches_post_action_skill_before_state() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill_with_hooks(
            "custom",
            "post-action-skill",
            "custom.post_action_skill",
            ProcMask::POST_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("post-action skill should register");
    let state = builder
        .register_state(
            "custom",
            "post-action-state",
            "custom.post_action_state",
            ProcMask::POST_ACTION,
            SkillPriority(0),
        )
        .expect("post-action state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 55,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_skill_handler(skill, skill_marks_update);
    runtime.set_state_handler(state, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("post-action skill and state plus attack should emit update");

    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "skill mark");
    assert_eq!(frame.updates.updates[1].score, skill.0);
    assert_eq!(frame.updates.updates[2].message, "state mark");
    assert_eq!(frame.updates.updates[2].score, 55);
}

#[test]
fn run_minimal_round_dispatches_late_post_action_skill_after_state() {
    let mut builder = ExtensionRegistryBuilder::default();
    let early = builder
        .register_skill_with_hooks(
            "custom",
            "early-post-action",
            "custom.early_post_action",
            ProcMask::POST_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("early post-action skill should register");
    let late = builder
        .register_skill_with_hooks_and_post_action_phase(
            "custom",
            "late-post-action",
            "custom.late_post_action",
            ProcMask::POST_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
            SkillPostActionPhase::Late,
        )
        .expect("late post-action skill should register");
    let state = builder
        .register_state(
            "custom",
            "post-action-state",
            "custom.post_action_state",
            ProcMask::POST_ACTION,
            SkillPriority(0),
        )
        .expect("post-action state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([late, early]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 55,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_skill_handler(early, skill_marks_update);
    runtime.set_skill_handler(late, skill_marks_update);
    runtime.set_state_handler(state, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("post-action hooks plus attack should emit update");

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![
            ("[0]攻击[1]", 3),
            ("skill mark", early.0),
            ("state mark", 55),
            ("skill mark", late.0),
        ]
    );
}

#[test]
fn run_minimal_round_dispatches_post_action_state_after_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let state = builder
        .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 77,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(state, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("attack plus state hook should emit update");

    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "state mark");
    assert_eq!(frame.updates.updates[1].score, 77);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
}

#[test]
fn run_minimal_round_flushes_post_action_state_effect_after_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let state = builder
        .register_state("custom", "regen", "custom.regen", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 88,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(state, state_pushes_nested_heal);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("attack plus state heal should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "[1]回复体力[2]点");
    assert_eq!(frame.updates.updates[1].score, 2);
}

#[test]
fn run_minimal_round_dispatches_damage_state_hooks_around_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let pre_damage = builder
        .register_state(
            "custom",
            "pre-damage",
            "custom.pre_damage",
            ProcMask::PRE_DAMAGE,
            SkillPriority(0),
        )
        .expect("pre-damage state should register");
    let post_damage = builder
        .register_state(
            "custom",
            "post-damage",
            "custom.post_damage",
            ProcMask::POST_DAMAGE,
            SkillPriority(0),
        )
        .expect("post-damage state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    {
        let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        store.add_entry(StateEntry {
            legacy_order_key: 11,
            extension_state_id: Some(pre_damage),
            hook_mask: ProcMask::PRE_DAMAGE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        store.add_entry(StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(post_damage),
            hook_mask: ProcMask::POST_DAMAGE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
    }
    runtime.set_state_handler(pre_damage, state_marks_update);
    runtime.set_state_handler(post_damage, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("damage state hooks plus attack should emit update");

    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "state mark");
    assert_eq!(frame.updates.updates[0].score, 11);
    assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[2].message, "state mark");
    assert_eq!(frame.updates.updates[2].score, 22);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
}

#[test]
fn run_minimal_round_flushes_post_damage_state_effect_before_post_action() {
    let mut builder = ExtensionRegistryBuilder::default();
    let post_damage = builder
        .register_state(
            "custom",
            "post-damage-regen",
            "custom.post_damage_regen",
            ProcMask::POST_DAMAGE,
            SkillPriority(0),
        )
        .expect("post-damage state should register");
    let post_action = builder
        .register_state(
            "custom",
            "post-action-marker",
            "custom.post_action_marker",
            ProcMask::POST_ACTION,
            SkillPriority(0),
        )
        .expect("post-action state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
    {
        let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        store.add_entry(StateEntry {
            legacy_order_key: 33,
            extension_state_id: Some(post_damage),
            hook_mask: ProcMask::POST_DAMAGE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        store.add_entry(StateEntry {
            legacy_order_key: 44,
            extension_state_id: Some(post_action),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
    }
    runtime.set_state_handler(post_damage, state_pushes_nested_heal);
    runtime.set_state_handler(post_action, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("post-damage effect plus post-action state should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "[1]回复体力[2]点");
    assert_eq!(frame.updates.updates[2].message, "state mark");
    assert_eq!(frame.updates.updates[2].score, 44);
}

#[test]
fn run_state_hooks_dispatches_registered_state_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let state = builder
        .register_state("custom", "burning", "custom.burning", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 42,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(state, state_marks_update);

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("state handler should emit update");

    assert_eq!(frame.updates.updates[0].message, "state mark");
    assert_eq!(frame.updates.updates[0].score, 42);
}

#[test]
fn run_state_hooks_exposes_controlled_rng_to_state_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let state = builder
        .register_state(
            "custom",
            "rng-state",
            "custom.rng_state",
            ProcMask::POST_ACTION,
            SkillPriority(0),
        )
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 88,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(state, state_consumes_rng);
    let mut expected_rng = RC4::default();
    let expected_value = expected_rng.next_i32(10);
    let expected_byte = expected_rng.next_u8();

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("state rng handler should emit update");

    assert_eq!(
        frame.updates.updates[0].message,
        format!("state-rng:{expected_value}:{expected_byte}")
    );
    assert_eq!(frame.updates.updates[0].score, expected_value as u32);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
}

#[test]
fn run_state_hooks_flushes_nested_effects_and_skips_legacy_entries() {
    let mut builder = ExtensionRegistryBuilder::default();
    let state = builder
        .register_state("custom", "regen", "custom.regen", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
    {
        let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        store.add_legacy_key(11);
        store.add_entry(StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
    }
    runtime.set_state_handler(state, state_pushes_nested_heal);

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("state heal should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
    assert_eq!(frame.updates.updates[0].score, 2);
}

#[test]
fn run_state_hooks_iron_post_action_decrements_step_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let iron_state = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
        registry,
    ));
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 300, 3, SkillPriority(10)));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(79)
            .and_then(StateEntry::iron_value),
        Some((300, 2))
    );
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
        2048
    );
}

#[test]
fn run_state_hooks_haste_post_action_decrements_step_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste_state = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(haste_state, run_haste_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::haste(
        77,
        haste_state,
        4,
        3,
        SkillPriority(100),
    ));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(77)
            .and_then(StateEntry::haste_value),
        Some((4, 2))
    );
}

#[test]
fn run_state_hooks_haste_post_action_clears_and_emits_release() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste_state = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(haste_state, run_haste_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::haste(
        77,
        haste_state,
        2,
        1,
        SkillPriority(100),
    ));

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("haste release should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(77), None);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(
        frame.updates.updates[0].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[1].message, "[1]从[疾走]中解除");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 0);
}

#[test]
fn run_state_hooks_haste_post_action_clears_dead_owner_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste_state = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;
    runtime.set_state_handler(haste_state, run_haste_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::haste(
        77,
        haste_state,
        2,
        1,
        SkillPriority(100),
    ));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(77), None);
}

#[test]
fn run_state_hooks_charm_post_action_decrements_step_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charm_state = builder
        .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("charm state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(charm_state, run_charm_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
        76,
        charm_state,
        7,
        Some(1),
        Some(2),
        Some(3),
        3,
        SkillPriority(100),
    ));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(76)
            .and_then(StateEntry::charm_value),
        Some((7, Some(1), Some(2), Some(3), 2))
    );
}

#[test]
fn run_state_hooks_charm_post_action_clears_and_emits_release() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charm_state = builder
        .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("charm state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(charm_state, run_charm_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
        76,
        charm_state,
        7,
        Some(1),
        Some(2),
        Some(3),
        1,
        SkillPriority(100),
    ));

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("charm release should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(76), None);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(
        frame.updates.updates[0].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[1].message, "[1]从[魅惑]中解除");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 0);
}

#[test]
fn run_state_hooks_charm_post_action_clears_dead_owner_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charm_state = builder
        .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("charm state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;
    runtime.set_state_handler(charm_state, run_charm_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
        76,
        charm_state,
        7,
        Some(1),
        Some(2),
        Some(3),
        1,
        SkillPriority(100),
    ));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(76), None);
}

#[test]
fn run_state_hooks_slow_post_action_decrements_step_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let slow_state = builder
        .register_state("core", "slow", "core.slow", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("slow state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(slow_state, run_slow_post_action_state);
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::slow(78, slow_state, 2, SkillPriority(100)));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(78)
            .and_then(StateEntry::slow_value),
        Some(1)
    );
}

#[test]
fn run_state_hooks_slow_post_action_clears_and_emits_release() {
    let mut builder = ExtensionRegistryBuilder::default();
    let slow_state = builder
        .register_state("core", "slow", "core.slow", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("slow state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(slow_state, run_slow_post_action_state);
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::slow(78, slow_state, 1, SkillPriority(100)));

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("slow release should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(78), None);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(
        frame.updates.updates[0].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[1].message, "[1]从[迟缓]中解除");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 0);
}

#[test]
fn run_skill_hooks_charge_post_action_decrements_step_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charge = builder
        .register_skill_with_hooks_and_post_action_phase(
            "core",
            "charge",
            "core.charge",
            ProcMask::POST_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
            SkillPostActionPhase::Late,
        )
        .expect("charge skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([charge])],
        registry,
    ));
    runtime.set_skill_handler(charge, run_charge_post_action_skill);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().activate_charge_runtime();

    let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::POST_ACTION);
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert!(frame.is_none());
    assert_eq!(
        owner.runtime.charge,
        crate::runtime_v2::entity::ChargeRuntime {
            active: true,
            post_action_active: true,
            step: 1,
        }
    );
    assert_eq!(owner.runtime.at_boost_millionths, 3_000_000);
}

#[test]
fn run_minimal_round_charge_late_post_action_clears_after_state_hooks() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charge = builder
        .register_skill_with_hooks_and_post_action_phase(
            "core",
            "charge",
            "core.charge",
            ProcMask::POST_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
            SkillPostActionPhase::Late,
        )
        .expect("charge skill should register");
    let state = builder
        .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([charge]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.activate_charge_runtime();
        owner.runtime.charge.step = 1;
        owner.states.add_entry(StateEntry {
            legacy_order_key: 55,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
    }
    runtime.set_skill_handler(charge, run_charge_post_action_skill);
    runtime.set_state_handler(state, state_marks_charge_boost);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("attack and charge-observing state should emit updates");
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert_eq!(
        frame.updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]攻击[1]", "charge boosted"]
    );
    assert_eq!(
        owner.runtime.charge,
        crate::runtime_v2::entity::ChargeRuntime {
            active: false,
            post_action_active: false,
            step: 0,
        }
    );
    assert_eq!(owner.runtime.at_boost_millionths, 1_000_000);
}

#[test]
fn run_skill_hooks_accumulate_activates_runtime_and_boosts_move() {
    let mut builder = ExtensionRegistryBuilder::default();
    let accumulate = builder
        .register_skill_with_hooks(
            "core",
            "accumulate",
            "core.accumulate",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("accumulate skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(100).with_skills([accumulate])],
        registry,
    ));
    runtime.set_skill_handler(accumulate, run_accumulate_skill);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("accumulate act should emit updates");
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[0]开始[聚气]", 1), ("[0]攻击力上升", 0)]
    );
    assert!(owner.runtime.accumulate.active);
    assert_eq!(owner.runtime.accumulate.charge_bonus(), 0.0);
    assert_eq!(owner.runtime.move_state.speed_points, 500);
    assert_eq!(owner.runtime.at_boost_millionths, 1_700_000);
}

#[test]
fn run_minimal_round_accumulate_uses_charge_bonus_until_late_charge_clear() {
    let mut builder = ExtensionRegistryBuilder::default();
    let accumulate = builder
        .register_skill_with_hooks(
            "core",
            "accumulate",
            "core.accumulate",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("accumulate skill should register");
    let charge = builder
        .register_skill_with_hooks_and_post_action_phase(
            "core",
            "charge",
            "core.charge",
            ProcMask::POST_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
            SkillPostActionPhase::Late,
        )
        .expect("charge skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3)
                .with_speed_points(100)
                .with_skills([accumulate, charge]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.activate_charge_runtime();
        owner.runtime.charge.step = 1;
    }
    runtime.set_skill_handler(accumulate, run_accumulate_skill);
    runtime.set_skill_handler(charge, run_charge_post_action_skill);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("accumulate, attack, and charge tick should emit frame");
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert_eq!(
        frame.updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]开始[聚气]", "[0]攻击力上升", "[0]攻击[1]"]
    );
    assert!(owner.runtime.accumulate.active);
    assert_eq!(owner.runtime.accumulate.charge_bonus(), 1.0);
    assert_eq!(owner.runtime.move_state.speed_points, 1000);
    assert_eq!(owner.runtime.charge.active, false);
    assert_eq!(owner.runtime.at_boost_millionths, 2_700_000);
}

#[test]
fn run_skill_hooks_clear_positive_runtime_orders_accumulate_before_charge() {
    let mut builder = ExtensionRegistryBuilder::default();
    let clear = builder
        .register_skill_with_hooks(
            "custom",
            "clear-positive",
            "custom.clear_positive",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("clear-positive skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.activate_charge_runtime();
        owner.activate_accumulate_runtime();
    }
    runtime.set_skill_handler(clear, skill_clears_positive_runtime);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("clear-positive runtime should emit messages");
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![(("[1]的[聚气]被打消了"), 100), (("[1]的[蓄力]被中止了"), 200)]
    );
    assert!(!owner.runtime.accumulate.active);
    assert!(!owner.runtime.charge.active);
    assert_eq!(owner.runtime.accumulate.acc(), 1.600000023841858);
    assert_eq!(owner.runtime.at_boost_millionths, 1_000_000);
}

#[test]
fn run_skill_hooks_clear_positive_states_removes_shield_and_orders_messages() {
    let mut builder = ExtensionRegistryBuilder::default();
    let clear = builder
        .register_skill_with_hooks(
            "custom",
            "clear-positive-states",
            "custom.clear_positive_states",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("clear-positive skill should register");
    let shield = builder
        .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
        .expect("shield state should register");
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let iron = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
        registry,
    ));
    {
        let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        store.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
        store.add_entry(StateEntry::shield(74, shield, 50, SkillPriority(6000)));
        store.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
    }
    runtime.set_skill_handler(clear, skill_clears_positive_states);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("clear-positive state messages should emit");
    let store = &runtime.entities.get(EntityIdx(0)).unwrap().states;

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[1]从[疾走]中解除", 300), ("[1]的[铁壁]被打消了", 400)]
    );
    assert_eq!(store.entry(74), None);
    assert_eq!(store.entry(77), None);
    assert_eq!(store.entry(79), None);
}

#[test]
fn run_skill_hooks_clear_positive_states_suppresses_dead_haste_message() {
    let mut builder = ExtensionRegistryBuilder::default();
    let clear = builder
        .register_skill_with_hooks(
            "custom",
            "clear-positive-states",
            "custom.clear_positive_states",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("clear-positive skill should register");
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let iron = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.runtime.alive = false;
        owner.states.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
        owner.states.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
    }
    runtime.set_skill_handler(clear, skill_clears_positive_states);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("iron clear message should emit");
    let store = &runtime.entities.get(EntityIdx(0)).unwrap().states;

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[1]的[铁壁]被打消了", 400)]
    );
    assert_eq!(store.entry(77), None);
    assert_eq!(store.entry(79), None);
}

#[test]
fn run_skill_hooks_clear_positive_combines_runtime_and_state_messages() {
    let mut builder = ExtensionRegistryBuilder::default();
    let clear = builder
        .register_skill_with_hooks(
            "custom",
            "clear-positive",
            "custom.clear_positive",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("clear-positive skill should register");
    let shield = builder
        .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
        .expect("shield state should register");
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let iron = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.activate_charge_runtime();
        owner.activate_accumulate_runtime();
        owner.states.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
        owner.states.add_entry(StateEntry::shield(74, shield, 50, SkillPriority(6000)));
        owner.states.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
    }
    runtime.set_skill_handler(clear, skill_clears_positive);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("combined clear-positive should emit messages");
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![
            ("[1]的[聚气]被打消了", 100),
            ("[1]的[蓄力]被中止了", 200),
            ("[1]从[疾走]中解除", 300),
            ("[1]的[铁壁]被打消了", 400),
        ]
    );
    assert!(!owner.runtime.accumulate.active);
    assert!(!owner.runtime.charge.active);
    assert_eq!(owner.runtime.accumulate.acc(), 1.600000023841858);
    assert_eq!(owner.runtime.at_boost_millionths, 1_000_000);
    assert_eq!(owner.states.entry(74), None);
    assert_eq!(owner.states.entry(77), None);
    assert_eq!(owner.states.entry(79), None);
}

#[test]
fn flush_effects_disperse_hit_clears_positives_and_spends_mp() {
    let mut builder = ExtensionRegistryBuilder::default();
    let shield = builder
        .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
        .expect("shield state should register");
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let iron = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3),
            PlayerTemplate::new(2, "target", 1, 10, 3).with_magic_point(96),
        ],
        registry,
    ));
    {
        let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        target.activate_charge_runtime();
        target.activate_accumulate_runtime();
        target.states.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
        target.states.add_entry(StateEntry::shield(74, shield, 50, SkillPriority(6000)));
        target.states.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
    }
    runtime.effects.push(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        damage: 1,
    });

    let frame = runtime.flush_effects().expect("disperse hit should emit clear-positive messages");
    let target = runtime.entities.get(EntityIdx(1)).unwrap();

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec![
            "[1]的[聚气]被打消了",
            "[1]的[蓄力]被中止了",
            "[1]从[疾走]中解除",
            "[1]的[铁壁]被打消了"
        ]
    );
    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .count(),
        4
    );
    assert_eq!(target.runtime.magic_point, 32);
    assert!(!target.runtime.accumulate.active);
    assert!(!target.runtime.charge.active);
    assert_eq!(target.runtime.at_boost_millionths, 1_000_000);
    assert_eq!(target.states.entry(74), None);
    assert_eq!(target.states.entry(77), None);
    assert_eq!(target.states.entry(79), None);
}

#[test]
fn flush_effects_disperse_hit_uses_legacy_mp_thresholds_and_skips_zero_damage() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3),
            PlayerTemplate::new(2, "high", 1, 10, 3).with_magic_point(65),
            PlayerTemplate::new(3, "mid", 1, 10, 3).with_magic_point(33),
            PlayerTemplate::new(4, "low", 1, 10, 3).with_magic_point(32),
            PlayerTemplate::new(5, "missed", 1, 10, 3).with_magic_point(96),
        ],
        ExtensionRegistry::default(),
    ));
    runtime.effects.push(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        damage: 1,
    });
    runtime.effects.push(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(2),
        damage: 1,
    });
    runtime.effects.push(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(3),
        damage: 1,
    });
    runtime.effects.push(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(4),
        damage: 0,
    });

    let frame = runtime.flush_effects();

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_point, 1);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_point, 0);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.magic_point, 0);
    assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().runtime.magic_point, 96);
}

#[test]
fn flush_effects_disperse_attack_doubles_atp_against_minion_targets() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies::default(),
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3).with_magic(80),
            PlayerTemplate::with_kind(2, "minion", minion_kind, 1, 10_000, 3).with_def_res(0, 16),
        ],
        registry,
    ));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 2.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let expected_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    runtime.effects.push(QueuedEffect::DisperseAttack {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    let frame = runtime.flush_effects().expect("minion disperse should emit damage");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[1].score, expected_amount as u32);
}

#[test]
fn run_state_hooks_iron_post_action_clears_and_emits_release() {
    let mut builder = ExtensionRegistryBuilder::default();
    let iron_state = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
        registry,
    ));
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 300, 1, SkillPriority(10)));

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("iron release should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(79), None);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
        1920
    );
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(
        frame.updates.updates[0].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[1].message, "[1]从[铁壁]中解除");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 0);
}

#[test]
fn run_state_hooks_iron_post_action_clears_expired_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let iron_state = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
        registry,
    ));
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 300, 0, SkillPriority(10)));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(79), None);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
        2048
    );
}

#[test]
fn run_state_hooks_iron_post_action_runs_at_legacy_priority() {
    let mut builder = ExtensionRegistryBuilder::default();
    let marker_state = builder
        .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("marker state should register");
    let iron_state = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 42,
        extension_state_id: Some(marker_state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(100),
        registration_order: RegistrationOrder(1),
        payload: StatePayload::None,
    });
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 300, 1, SkillPriority(10)));
    runtime.set_state_handler(marker_state, state_marks_update);
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);

    let plan = runtime.scheduler.state_hook_plan(&runtime.entities, EntityIdx(0), ProcMask::POST_ACTION);
    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("marker and iron release should emit updates");

    assert_eq!(
        plan.entries
            .iter()
            .map(|entry| (entry.legacy_order_key, entry.priority))
            .collect::<Vec<_>>(),
        vec![(42, SkillPriority(100)), (79, SkillPriority(210))]
    );
    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "state mark");
    assert_eq!(
        frame.updates.updates[1].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[2].message, "[1]从[铁壁]中解除");
}

#[test]
fn run_state_hooks_haste_charm_slow_and_iron_share_legacy_post_action_priority() {
    let mut builder = ExtensionRegistryBuilder::default();
    let marker_state = builder
        .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("marker state should register");
    let poison_state = builder
        .register_state("core", "poison", "core.poison", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("poison state should register");
    let haste_state = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let charm_state = builder
        .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("charm state should register");
    let slow_state = builder
        .register_state("core", "slow", "core.slow", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("slow state should register");
    let iron_state = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
        registry,
    ));
    {
        let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        store.add_entry(StateEntry {
            legacy_order_key: 42,
            extension_state_id: Some(marker_state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(100),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
        store.add_entry(StateEntry::poison(
            75,
            poison_state,
            Some(0),
            Some(0),
            80.0,
            2,
            SkillPriority(0),
        ));
        store.add_entry(StateEntry::haste(77, haste_state, 2, 1, SkillPriority(100)));
        store.add_entry(StateEntry::charm(
            76,
            charm_state,
            7,
            Some(1),
            Some(2),
            Some(3),
            1,
            SkillPriority(100),
        ));
        store.add_entry(StateEntry::slow(78, slow_state, 1, SkillPriority(100)));
        store.add_entry(StateEntry::iron(79, iron_state, 300, 1, SkillPriority(10)));
    }
    runtime.set_state_handler(marker_state, state_marks_update);
    runtime.set_state_handler(poison_state, run_poison_post_action_state);
    runtime.set_state_handler(haste_state, run_haste_post_action_state);
    runtime.set_state_handler(charm_state, run_charm_post_action_state);
    runtime.set_state_handler(slow_state, run_slow_post_action_state);
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);

    let plan = runtime.scheduler.state_hook_plan(&runtime.entities, EntityIdx(0), ProcMask::POST_ACTION);
    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("marker and timed release states should emit updates");

    assert_eq!(
        plan.entries
            .iter()
            .map(|entry| (entry.legacy_order_key, entry.priority))
            .collect::<Vec<_>>(),
        vec![
            (42, SkillPriority(100)),
            (75, SkillPriority(150)),
            (77, SkillPriority(210)),
            (76, SkillPriority(210)),
            (78, SkillPriority(210)),
            (79, SkillPriority(210)),
        ]
    );
    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec![
            "state mark",
            "[1][毒性发作]",
            "[1]受到[2]点伤害",
            "[1]从[疾走]中解除",
            "[1]从[魅惑]中解除",
            "[1]从[迟缓]中解除",
            "[1]从[铁壁]中解除",
        ]
    );
}

#[test]
fn flush_effects_dispatches_custom_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let marker = builder
        .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler(marker, custom_marks_update);

    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        marker,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Text("custom mark".to_owned()),
    )));

    let frame = runtime.flush_effects().expect("custom handler should emit update");
    assert_eq!(frame.updates.updates[0].message, "custom mark");
}

#[test]
fn flush_effects_exposes_controlled_rng_to_custom_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let rng_handler = builder
        .register_effect_handler("custom", "rng", "custom.rng", SkillPriority(0))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler(rng_handler, custom_consumes_rng);
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        rng_handler,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Int(10),
    )));
    let mut expected_rng = RC4::default();
    let expected_value = expected_rng.next_i32(10);
    let expected_byte = expected_rng.next_u8();

    let frame = runtime.flush_effects().expect("custom rng handler should emit update");

    assert_eq!(
        frame.updates.updates[0].message,
        format!("rng:{expected_value}:{expected_byte}")
    );
    assert_eq!(frame.updates.updates[0].score, expected_value as u32);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
}

#[test]
fn flush_effects_runs_nested_custom_effect_before_older_siblings() {
    let mut builder = ExtensionRegistryBuilder::default();
    let nested_damage = builder
        .register_effect_handler("custom", "nested-damage", "custom.nested_damage", SkillPriority(0))
        .expect("handler should register");
    let marker = builder
        .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(1))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler(nested_damage, custom_spawns_nested_damage);
    runtime.set_effect_handler(marker, custom_marks_update);

    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        nested_damage,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Int(4),
    )));
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        marker,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Text("after nested".to_owned()),
    )));

    let frame = runtime.flush_effects().expect("nested damage should emit update");
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 6);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "after nested");
}

#[test]
fn flush_effects_applies_heal_without_exceeding_max_hp() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 4;
    runtime.effects.push(QueuedEffect::Heal {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        amount: 20,
    });

    let frame = runtime.flush_effects().expect("heal should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
    assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
}

#[test]
fn flush_effects_readds_healed_dead_target_to_alive_views() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 4, 3));
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        amount: 4,
    });
    runtime.flush_effects().expect("lethal damage should emit update");
    runtime.effects.push(QueuedEffect::Heal {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        amount: 2,
    });

    runtime.flush_effects().expect("heal should emit update");

    assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 2);
    assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1)].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert_eq!(
        runtime.world.first_alive_enemy(EntityIdx(0), &runtime.entities),
        Some(EntityIdx(1))
    );
}

#[test]
fn flush_effects_spawns_entity_and_adds_it_to_round_order() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::new(3, "summoned", 0, 5, 2),
    });

    let frame = runtime.flush_effects().expect("spawn should emit update");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().template.name, "summoned");
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.owner, EntityIdx(0));
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.root_owner, EntityIdx(0));
    assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(2), EntityIdx(1)]);
    assert_eq!(frame.updates.updates[0].message, "出现一个新的[1]");
}

#[test]
fn owner_spawn_ignores_blueprint_team_and_does_not_create_enemy() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        ExtensionRegistry::default(),
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::new(2, "enemy", 1, 8, 4),
    });
    runtime.flush_effects().expect("spawn should emit update");

    let spawned = runtime.entities.get(EntityIdx(1)).expect("spawned entity should exist");
    assert_eq!(spawned.template.team, 0);
    assert_eq!(spawned.runtime.team, 0);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(1)].as_slice()));
    assert_eq!(runtime.world.sync_winner(&runtime.entities), Some(0));
    assert_eq!(
        runtime.scheduler.select_minimal_action(&mut runtime.world, &runtime.entities),
        None
    );
}

#[test]
fn flush_effects_adds_and_clears_state_entries() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    let state = StateEntry {
        legacy_order_key: 77,
        extension_state_id: Some(StateId(1)),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(5),
        registration_order: RegistrationOrder(2),
        payload: StatePayload::None,
    };

    runtime.effects.push(QueuedEffect::AddState {
        target: EntityIdx(1),
        state: state.clone(),
    });
    let add_frame = runtime.flush_effects().expect("add state should emit update");
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), Some(&state));
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.generation(), 1);
    assert_eq!(add_frame.updates.updates[0].message, "[1]状态改变");

    runtime.effects.push(QueuedEffect::ClearState {
        target: EntityIdx(1),
        legacy_order_key: 77,
    });
    let clear_frame = runtime.flush_effects().expect("clear state should emit update");
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), None);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.generation(), 2);
    assert_eq!(clear_frame.updates.updates[0].message, "[1]状态解除");
}

#[test]
fn flush_effects_runs_nested_heal_before_older_siblings() {
    let mut builder = ExtensionRegistryBuilder::default();
    let nested_heal = builder
        .register_effect_handler("custom", "nested-heal", "custom.nested_heal", SkillPriority(0))
        .expect("handler should register");
    let marker = builder
        .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(1))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 3;
    runtime.set_effect_handler(nested_heal, custom_spawns_nested_heal);
    runtime.set_effect_handler(marker, custom_marks_update);

    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        nested_heal,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Int(4),
    )));
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        marker,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Text("after heal".to_owned()),
    )));

    let frame = runtime.flush_effects().expect("nested heal should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
    assert_eq!(frame.updates.updates[1].message, "after heal");
}

#[test]
fn flush_effects_revives_dead_entity_with_capped_hp() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.world.remove_round_actor(EntityIdx(1));
    let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
    target.runtime.hp = 0;
    target.runtime.alive = false;
    runtime.effects.push(QueuedEffect::Revive {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        hp: 20,
    });

    let frame = runtime.flush_effects().expect("revive should emit update");

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(target.runtime.hp, 10);
    assert!(target.runtime.alive);
    assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1)]);
    assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1)].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
    assert_eq!(frame.updates.updates[0].message, "[1][复活]了");
}

#[test]
fn flush_effects_removes_entity_from_alive_set() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.effects.push(QueuedEffect::Remove {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    let frame = runtime.flush_effects().expect("remove should emit update");

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(target.runtime.hp, 0);
    assert!(!target.runtime.alive);
    assert_eq!(runtime.world.round_order(), &[EntityIdx(0)]);
    assert_eq!(runtime.world.team_alive(1), Some([].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0)]);
    assert_eq!(frame.updates.updates[0].message, "[1]消失了");
}

#[test]
fn flush_effects_emits_replay_effect_without_state_mutation() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.effects.push(QueuedEffect::Replay {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        message: "custom replay".to_owned(),
        score: 7,
    });

    let frame = runtime.flush_effects().expect("replay should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
    assert_eq!(frame.updates.updates[0].message, "custom replay");
    assert_eq!(frame.updates.updates[0].score, 7);
}

#[test]
fn flush_effects_merges_fixed_lane_skill_loadout() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill_a = builder
        .register_skill("custom", "a", "custom.a", TargetPolicy::Enemy, SkillPriority(0))
        .expect("skill should register");
    let skill_b = builder
        .register_skill("custom", "b", "custom.b", TargetPolicy::Enemy, SkillPriority(1))
        .expect("skill should register");
    let skill_c = builder
        .register_skill("custom", "c", "custom.c", TargetPolicy::Enemy, SkillPriority(2))
        .expect("skill should register");
    let merge_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "merge",
            "custom.merge",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: false,
            },
        )
        .expect("merge kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "left", merge_kind, 0, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_a, 1)])),
            PlayerTemplate::new(2, "right", 1, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_b, 2), (skill_c, 3)])),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Merge {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    let frame = runtime.flush_effects().expect("merge should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill_a]);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.levels(), &[2]);
    assert_eq!(
        frame.updates.updates[0].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[1].message, "[0][吞噬]了[1]");
    assert_eq!(frame.updates.updates[1].score, 60);
    assert_eq!(frame.updates.updates[2].message, "[0]属性上升");
    assert_eq!(frame.updates.updates[2].score, 0);
}

#[test]
fn flush_effects_merge_drops_unmapped_skills_when_policy_requires() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill_a = builder
        .register_skill("custom", "a", "custom.a", TargetPolicy::Enemy, SkillPriority(0))
        .expect("skill should register");
    let skill_b = builder
        .register_skill("custom", "b", "custom.b", TargetPolicy::Enemy, SkillPriority(1))
        .expect("skill should register");
    let skill_c = builder
        .register_skill("custom", "c", "custom.c", TargetPolicy::Enemy, SkillPriority(2))
        .expect("skill should register");
    let merge_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "merge",
            "custom.merge",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::DropUnmappedSkills,
                inherit_owner_def_res: false,
            },
        )
        .expect("merge kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "left", merge_kind, 0, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_a, 1)]).with_fixed_lane_keys([1])),
            PlayerTemplate::new(2, "right", 1, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_b, 2), (skill_c, 3)])),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Merge {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    runtime.flush_effects().expect("merge should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill_a]);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.levels(), &[3]);
}

#[test]
fn flush_effects_panics_on_unknown_damage_target() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        amount: 1,
    });

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

    assert!(result.is_err());
}

fn assert_effect_panics(effect: QueuedEffect) {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.effects.push(effect);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

    assert!(result.is_err());
}

fn dummy_state_entry() -> StateEntry {
    StateEntry {
        legacy_order_key: 999,
        extension_state_id: None,
        hook_mask: ProcMask::NONE,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    }
}

#[test]
fn flush_effects_panics_on_unknown_effect_entities() {
    assert_effect_panics(QueuedEffect::Damage {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        amount: 1,
    });
    assert_effect_panics(QueuedEffect::SummonExplode {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        fire_state_key: 91,
    });
    assert_effect_panics(QueuedEffect::SummonExplode {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        fire_state_key: 91,
    });
    assert_effect_panics(QueuedEffect::DisperseAttack {
        caster: EntityIdx(99),
        target: EntityIdx(1),
    });
    assert_effect_panics(QueuedEffect::DisperseAttack {
        caster: EntityIdx(0),
        target: EntityIdx(99),
    });
    assert_effect_panics(QueuedEffect::DisperseHit {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        damage: 1,
    });
    assert_effect_panics(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        damage: 1,
    });
    assert_effect_panics(QueuedEffect::Heal {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        amount: 1,
    });
    assert_effect_panics(QueuedEffect::Heal {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        amount: 1,
    });
    assert_effect_panics(QueuedEffect::Spawn {
        caster: EntityIdx(99),
        template: PlayerTemplate::new(3, "ghost", 1, 1, 0),
    });
    assert_effect_panics(QueuedEffect::AddState {
        target: EntityIdx(99),
        state: dummy_state_entry(),
    });
    assert_effect_panics(QueuedEffect::ClearState {
        target: EntityIdx(99),
        legacy_order_key: 999,
    });
    assert_effect_panics(QueuedEffect::Revive {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        hp: 1,
    });
    assert_effect_panics(QueuedEffect::Revive {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        hp: 1,
    });
    assert_effect_panics(QueuedEffect::Remove {
        caster: EntityIdx(99),
        target: EntityIdx(1),
    });
    assert_effect_panics(QueuedEffect::Remove {
        caster: EntityIdx(0),
        target: EntityIdx(99),
    });
    assert_effect_panics(QueuedEffect::Merge {
        caster: EntityIdx(99),
        target: EntityIdx(1),
    });
    assert_effect_panics(QueuedEffect::Merge {
        caster: EntityIdx(0),
        target: EntityIdx(99),
    });
    assert_effect_panics(QueuedEffect::Replay {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        message: "bad caster".to_owned(),
        score: 0,
    });
    assert_effect_panics(QueuedEffect::Replay {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        message: "bad target".to_owned(),
        score: 0,
    });
}

#[test]
fn flush_effects_panics_on_unknown_custom_effect_entities() {
    let mut builder = ExtensionRegistryBuilder::default();
    let handler = builder
        .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler(handler, custom_marks_update);
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        handler,
        EntityIdx(99),
        Some(EntityIdx(1)),
        CustomEffectPayload::Text("bad caster".to_owned()),
    )));

    let bad_caster = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

    assert!(bad_caster.is_err());

    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        handler,
        EntityIdx(0),
        Some(EntityIdx(99)),
        CustomEffectPayload::Text("bad target".to_owned()),
    )));

    let bad_target = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

    assert!(bad_target.is_err());
}

#[test]
fn custom_context_restricts_cross_entity_reads_by_capability() {
    let mut builder = ExtensionRegistryBuilder::default();
    let reader = builder
        .register_effect_handler("custom", "reader", "custom.reader", SkillPriority(0))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
            PlayerTemplate::new(3, "third", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler(reader, custom_rejects_cross_entity_read);
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        reader,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::None,
    )));
    let denied = runtime.flush_effects().expect("denied read handler should emit update");
    assert_eq!(denied.updates.updates[0].message, "read denied");

    runtime.set_effect_handler_with_capabilities(reader, custom_reads_cross_entity, &[ExtensionCapability::ReadEnemies]);
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        reader,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::None,
    )));
    let allowed = runtime.flush_effects().expect("allowed read handler should emit update");
    assert_eq!(allowed.updates.updates[0].message, "third");
}

#[test]
fn custom_context_requires_capability_for_entity_slot_mutation() {
    let mut builder = ExtensionRegistryBuilder::default();
    let slot = builder
        .reserve_entity_slot("custom", "flag", "custom.flag")
        .expect("entity slot should reserve");
    let mutator = builder
        .register_effect_handler("custom", "mutator", "custom.mutator", SkillPriority(0))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler_with_capabilities(mutator, custom_mutates_entity_slot, &[ExtensionCapability::MutateEntitySlots]);
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        mutator,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Int(slot.0 as i32),
    )));

    let frame = runtime.flush_effects().expect("slot mutation should emit update");

    assert_eq!(frame.updates.updates[0].message, "slot set");
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().slots.get(slot),
        Some(&SlotValue::Bool(true))
    );
}

#[test]
fn runtime_dispatches_replay_renderers_in_registry_order() {
    let mut builder = ExtensionRegistryBuilder::default();
    let late = builder
        .register_replay_renderer("custom", "late", "custom.late_replay", SkillPriority(10))
        .expect("late replay renderer should register");
    let early = builder
        .register_replay_renderer("custom", "early", "custom.early_replay", SkillPriority(1))
        .expect("early replay renderer should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_replay_renderer(late, render_update_count_replay);
    runtime.set_replay_renderer(early, render_first_message_replay);
    let frame = RuntimeFrame::single_damage(0, 0, 3);

    let rendered = runtime.render_replay_frame(&frame);

    assert_eq!(
        rendered,
        vec![
            RenderedReplay::new(ReplayRendererId(0), "[0]攻击[1]"),
            RenderedReplay::new(ReplayRendererId(1), "1")
        ]
    );
}

#[test]
fn runtime_dispatches_show_renderers_in_registry_order() {
    let mut builder = ExtensionRegistryBuilder::default();
    let show = builder
        .register_show_renderer("custom", "show", "custom.show", SkillPriority(0))
        .expect("show renderer should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_show_renderer(show, render_first_message_show);
    let frame = RuntimeFrame::single_damage(0, 0, 3);

    let rendered = runtime.render_show_frame(&frame);

    assert_eq!(rendered, vec![RenderedShow::new(ShowRendererId(0), "[0]攻击[1]")]);
}

#[test]
fn runtime_dispatches_hp_marker_show_renderer_golden() {
    let mut builder = ExtensionRegistryBuilder::default();
    let show = builder
        .register_show_renderer("custom", "hp-marker", "custom.hp_marker.show", SkillPriority(0))
        .expect("hp marker show renderer should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_show_renderer(show, render_hp_marker_bar_show);
    let mut updates = crate::engine::update::RunUpdates::new();
    let mut hp_report = RuntimeFrame::replay_update(0, 0, "[0]还剩[2]点血", 0);
    hp_report.param = Some(87);
    updates.add(hp_report);
    let frame = RuntimeFrame { updates };

    let rendered = runtime.render_show_frame(&frame);

    assert_eq!(
        rendered,
        vec![RenderedShow::new(ShowRendererId(0), "hp-bar:actor=0:value=87:text=0还剩87点血")]
    );
}

#[test]
fn runtime_frame_renders_core_replay_and_show_golden() {
    let mut frame = RuntimeFrame::single_damage(0, 1, 3);
    frame.updates.add(RuntimeFrame::replay_update(0, 1, "[0]属性上升", 0));

    assert_eq!(
        frame.render_core_replay(),
        vec![
            CoreReplayEvent {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 3,
            },
            CoreReplayEvent {
                message: "[0]属性上升".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 0,
            },
        ]
    );
    assert_eq!(
        frame.render_core_show(),
        vec![
            CoreShowEvent {
                text: "0攻击1".to_owned(),
                score: 3,
            },
            CoreShowEvent {
                text: "0属性上升".to_owned(),
                score: 0,
            },
        ]
    );
}

#[test]
fn runtime_frame_renders_hp_marker_core_show_golden() {
    let mut updates = crate::engine::update::RunUpdates::new();
    let mut hp_report = RuntimeFrame::replay_update(0, 0, "[0]还剩[2]点血", 0);
    hp_report.param = Some(87);
    updates.add(hp_report);
    let frame = RuntimeFrame { updates };

    assert_eq!(
        frame.render_core_replay(),
        vec![CoreReplayEvent {
            message: "[0]还剩[2]点血".to_owned(),
            caster: 0,
            target: 0,
            targets: Vec::new(),
            param: Some(87),
            score: 0,
        }]
    );
    assert_eq!(
        frame.render_core_show(),
        vec![CoreShowEvent {
            text: "0还剩87点血".to_owned(),
            score: 0,
        }]
    );
}

#[test]
fn plain_absorb_smart_low_missing_hp_skips_probability_rng() {
    let registry = ExtensionRegistryBuilder::default().build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "actor", 0, 100, 3),
            PlayerTemplate::new(2, "target", 1, 100, 3),
        ],
        registry,
    ));
    let expected_rng = runtime.rng.clone();

    assert!(!runtime.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Absorb, 128, true));
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn plain_accumulate_gates_skip_probability_rng() {
    let registry = ExtensionRegistryBuilder::default().build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "actor", 0, 200, 3),
            PlayerTemplate::new(2, "target", 1, 100, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 119;
    let expected_rng = runtime.rng.clone();

    assert!(!runtime.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Accumulate, 128, true));
    assert_rng_state_eq(&runtime.rng, &expected_rng);

    {
        let actor = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        actor.runtime.hp = actor.template.max_hp;
        assert!(actor.activate_accumulate_runtime());
    }
    assert!(!runtime.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Accumulate, 128, false));
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn plain_curse_empty_smart_targets_still_consume_sampling_rng() {
    let registry = ExtensionRegistryBuilder::default().build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "actor", 0, 100, 3),
            PlayerTemplate::new(2, "low-hp-target", 1, 100, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 79;
    let all_alive = runtime.world.flat_alive().to_vec();
    let mut expected_rng = runtime.rng.clone();
    for _ in 0..7 {
        assert_eq!(expected_rng.pick_skip_range(&all_alive, &[0]), Some(1));
    }

    assert!(runtime.select_plain_curse_targets(EntityIdx(0), true).is_empty());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn reflect_failed_level_roll_only_consumes_r255() {
    let mut builder = ExtensionRegistryBuilder::default();
    let reflect = builder
        .register_skill_with_hooks(
            "core",
            "reflect",
            DEFAULT_CORE_REFLECT_SKILL_EXPORT,
            ProcMask::PRE_DEFEND,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("reflect skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            PlayerTemplate::new(2, "reflector", 1, 100, 3).with_skill_loadout(SkillLoadout::from_skill_levels([(reflect, 1)])),
        ],
        registry,
    ));
    runtime.set_skill_handler(reflect, run_reflect_pre_defend_skill);
    let mut expected_rng = runtime.rng.clone();
    expected_rng.r255();
    let mut updates = RunUpdates::new();
    let mut defend_value = RuntimeDefendValue::Atp {
        value: 50.0,
        caster: EntityIdx(0),
        target: EntityIdx(1),
        is_magic: true,
    };

    runtime.drain_pre_defend_hooks_into(EntityIdx(1), &mut updates, &mut defend_value);

    assert_eq!(defend_value.atp(), Some(50.0));
    assert!(updates.updates.is_empty());
    assert!(runtime.effects.is_empty());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn reflected_attack_applies_damage_before_move_penalty_finishes() {
    let registry = ExtensionRegistryBuilder::default().build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "reflector", 0, 100, 3)
                .with_magic(10_000)
                .with_wisdom(10_000)
                .with_speed_points(1_000),
            PlayerTemplate::new(2, "target", 1, 1_000, 3).with_def_res(0, 16),
        ],
        registry,
    ));
    while {
        let mut probe = runtime.rng.clone();
        probe.next_u8() <= 7
    } {
        runtime.rng.next_u8();
    }
    runtime.effects.push(QueuedEffect::ReflectedAttack {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        atp_bits: 50.0_f64.to_bits(),
    });

    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
        1_000
    );
    let frame = runtime.flush_effects().expect("reflected attack should emit damage");

    assert!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp < 1_000,
        "reflected damage must resolve before the queued effect completes"
    );
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 520);
    assert!(
        frame
            .updates
            .updates
            .iter()
            .any(|update| update.caster == 0 && update.target == 1 && update.score > 0)
    );
}

#[test]
fn plain_curse_skill_applies_state_after_damage() {
    let mut builder = ExtensionRegistryBuilder::default();
    let curse_state = builder
        .register_state(
            "core",
            "curse",
            DEFAULT_CORE_CURSE_STATE_EXPORT,
            ProcMask::POST_DEFEND,
            SkillPriority(10_000),
        )
        .expect("curse state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_magic(80).with_wisdom(64),
            PlayerTemplate::new(2, "target", 1, 1_000, 3)
                .with_def_res(0, 16)
                .with_target_score_stats(0, 7, 1.0),
        ],
        registry,
    ));
    runtime.set_state_handler(curse_state, run_curse_post_defend_state);
    while {
        let mut probe = runtime.rng.clone();
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut probe);
        PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut probe,
        )
    } {
        runtime.rng.next_u8();
    }
    let mut updates = RunUpdates::new();

    runtime.drain_plain_curse_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert!((1..1_000).contains(&target.runtime.hp));
    assert_eq!(target.runtime.atk_sum, 28);
    assert_eq!(
        target.states.entry(PLAIN_CURSE_STATE_KEY).map(|entry| entry.payload.clone()),
        Some(StatePayload::Curse { prob: 42, multiply: 2 })
    );
    assert_eq!(updates.updates.first().unwrap().message, "[0]使用[诅咒]");
    assert!(updates.updates[1].message.starts_with("[1]受到[2]点伤害"));
    assert_eq!(updates.updates.last().unwrap().message, "[1]被[诅咒]了");
}

#[test]
fn plain_curse_on_damage_stacks_charge_bonus_without_reapplying_atk_sum() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .register_state(
            "core",
            "curse",
            DEFAULT_CORE_CURSE_STATE_EXPORT,
            ProcMask::POST_DEFEND,
            SkillPriority(10_000),
        )
        .expect("curse state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            PlayerTemplate::new(2, "charged-target", 1, 100, 3)
                .with_at_boost_millionths(3_000_000)
                .with_target_score_stats(0, 7, 1.0),
        ],
        registry,
    ));
    let mut updates = RunUpdates::new();

    runtime.apply_curse_on_damage(EntityIdx(0), EntityIdx(1), 1, &mut updates);
    runtime.apply_curse_on_damage(EntityIdx(0), EntityIdx(1), 1, &mut updates);

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(target.runtime.atk_sum, 28);
    assert_eq!(
        target.states.entry(PLAIN_CURSE_STATE_KEY).map(|entry| entry.payload.clone()),
        Some(StatePayload::Curse { prob: 72, multiply: 5 })
    );
    assert_eq!(
        updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[1]被[诅咒]了", 60), ("[1]被[诅咒]了", 60)]
    );
}

#[test]
fn plain_default_enemy_target_selection_matches_legacy_rng_for_single_enemy() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "caster", 0, 100, 3),
        PlayerTemplate::new(2, "ally", 0, 100, 3),
        PlayerTemplate::new(3, "enemy", 1, 100, 3),
    ]));
    let all_alive = runtime.world.flat_alive().to_vec();
    let mut expected_rng = runtime.rng.clone();
    for _ in 0..4 {
        assert_eq!(expected_rng.pick_skip_range(&all_alive, &[0, 1]), Some(2));
    }
    let _ = expected_rng.rFFFF();

    let selected = runtime.select_plain_default_enemy_targets(EntityIdx(0), false);

    assert_eq!(selected, vec![EntityIdx(2)]);
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn plain_poison_static_dispatch_applies_threshold_and_stacking_semantics() {
    let mut builder = ExtensionRegistryBuilder::default();
    let poison = builder
        .register_skill(
            "core",
            "poison",
            BuiltinActiveSkill::Poison.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(5),
        )
        .expect("poison skill should register");
    let poison_state = builder
        .register_state(
            "core",
            "poison",
            DEFAULT_CORE_POISON_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(150),
        )
        .expect("poison state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3)
                .with_magic(1_000_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(poison, 128)])),
            PlayerTemplate::new(2, "target", 1, 100_000, 3).with_def_res(0, 0),
        ],
        registry,
    ));
    let mut updates = RunUpdates::new();
    let threshold_rng = runtime.rng.clone();

    runtime.apply_poison_on_damage(EntityIdx(0), EntityIdx(1), 4, &mut updates);

    assert_rng_state_eq(&runtime.rng, &threshold_rng);
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().states.entry(PLAIN_POISON_STATE_KEY),
        None
    );
    assert!(updates.updates.is_empty());

    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("poison should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Poison);
    assert_eq!(prepared.targets, vec![EntityIdx(1)]);

    let mut expected_rng = runtime.rng.clone();
    let attack_atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng,
    ));
    let damage = (attack_atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    assert!(damage > 4);
    let first_poison_atp =
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 1.2000000476837158;

    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(PLAIN_POISON_STATE_KEY)
            .and_then(StateEntry::poison_value),
        Some((Some(0), Some(1), first_poison_atp, 4))
    );
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0][投毒]", "[1]受到[2]点伤害[s_dmg160]", "[1][中毒]"]
    );

    let mut expected_rng = runtime.rng.clone();
    let second_poison_atp =
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 1.2000000476837158;

    runtime.apply_poison_on_damage(EntityIdx(0), EntityIdx(1), 5, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(
        target.states.entry(PLAIN_POISON_STATE_KEY).and_then(StateEntry::poison_value),
        Some((Some(0), Some(1), first_poison_atp + second_poison_atp, 4))
    );
    assert_eq!(
        target.states.entry(PLAIN_POISON_STATE_KEY).and_then(|entry| entry.extension_state_id),
        Some(poison_state)
    );
    assert_eq!(updates.updates.last().unwrap().message, "[1][中毒]");
}

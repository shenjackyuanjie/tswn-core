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

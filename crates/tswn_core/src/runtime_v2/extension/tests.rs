use super::*;

struct FixtureExtension;

impl TswnExtension for FixtureExtension {
    fn name(&self) -> &'static str { "fixture" }

    fn version(&self) -> ExtensionVersion { ExtensionVersion::new(1, 2, 3) }

    fn capabilities(&self) -> &'static [ExtensionCapability] {
        &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots]
    }

    fn register(&self, registry: &mut ExtensionRegistryBuilder) -> Result<(), ExtensionError> {
        registry.register_player_kind("fixture", "kind", "fixture.kind")?;
        registry.register_skill("fixture", "skill", "fixture.skill", TargetPolicy::Enemy, SkillPriority(3))?;
        registry.reserve_entity_slot("fixture", "flags", "fixture.flags")?;
        Ok(())
    }
}

#[test]
fn registry_allocates_stable_player_kind_ids_in_registration_order() {
    let mut builder = ExtensionRegistryBuilder::default();

    let alpha = builder
        .register_player_kind("custom", "alpha", "custom.alpha")
        .expect("alpha should register");
    let beta = builder
        .register_player_kind("custom", "beta", "custom.beta")
        .expect("beta should register");

    assert_eq!(alpha, PlayerKindId(0));
    assert_eq!(beta, PlayerKindId(1));

    let registry = builder.build();
    assert_eq!(registry.player_kind(alpha).unwrap().name, "alpha");
    assert_eq!(registry.player_kind(beta).unwrap().export_name, "custom.beta");
    assert_eq!(registry.player_kind(alpha).unwrap().flags, PlayerKindFlags::NONE);
    assert_eq!(registry.player_kind(alpha).unwrap().policies, PlayerKindPolicies::default());
    assert_eq!(registry.player_kinds().len(), 2);
}

#[test]
fn registry_stores_player_kind_policy_flags_without_kind_explosion() {
    let mut builder = ExtensionRegistryBuilder::default();

    let bed2 = builder
        .register_player_kind_with_policies(
            "custom",
            "bed2",
            "custom.bed2",
            PlayerKindFlags::BED2 | PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: true,
            },
        )
        .expect("bed2 kind should register");

    let registry = builder.build();
    let spec = registry.player_kind(bed2).expect("bed2 kind should exist");
    assert!(spec.flags.contains(PlayerKindFlags::BED2));
    assert!(spec.flags.contains(PlayerKindFlags::SUMMON));
    assert_eq!(spec.policies.owner_resolution, OwnerResolutionPolicy::RootOwner);
    assert_eq!(spec.policies.damage_share, DamageSharePolicy::ShareToOwner);
    assert_eq!(spec.policies.merge, MergePolicy::FixedLane);
    assert!(spec.policies.inherit_owner_def_res);
    assert_eq!(registry.player_kinds().len(), 1);
}

#[test]
fn registry_allows_same_local_name_in_different_namespaces() {
    let mut builder = ExtensionRegistryBuilder::default();

    let custom = builder
        .register_player_kind("custom", "boss", "custom.boss")
        .expect("custom boss should register");
    let fixture = builder
        .register_player_kind("fixture", "boss", "fixture.boss")
        .expect("fixture boss should register");

    assert_eq!(custom, PlayerKindId(0));
    assert_eq!(fixture, PlayerKindId(1));
}

#[test]
fn registry_rejects_duplicate_player_kind_name_in_namespace() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .register_player_kind("custom", "boss", "custom.boss")
        .expect("first boss should register");

    assert_eq!(
        builder.register_player_kind("custom", "boss", "custom.boss.v2"),
        Err(ExtensionError::DuplicateName {
            namespace: "custom".to_owned(),
            name: "boss".to_owned(),
        })
    );
}

#[test]
fn registry_rejects_duplicate_export_name() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .register_player_kind("custom", "boss", "custom.boss")
        .expect("first boss should register");

    assert_eq!(
        builder.register_player_kind("fixture", "boss", "custom.boss"),
        Err(ExtensionError::DuplicateExportName {
            export_name: "custom.boss".to_owned(),
        })
    );
}

#[test]
fn registry_stores_skill_and_state_specs() {
    let mut builder = ExtensionRegistryBuilder::default();

    let skill = builder
        .register_skill("custom", "fire", "custom.fire", TargetPolicy::Enemy, SkillPriority(10))
        .expect("skill should register");
    let state = builder
        .register_state(
            "custom",
            "burning",
            "custom.burning",
            ProcMask::POST_ACTION | ProcMask::POST_DAMAGE,
            SkillPriority(5),
        )
        .expect("state should register");

    assert_eq!(skill, SkillId(0));
    assert_eq!(state, StateId(0));

    let registry = builder.build();
    assert_eq!(registry.skill(skill).unwrap().target_policy, TargetPolicy::Enemy);
    assert_eq!(
        registry.state(state).unwrap().hook_mask,
        ProcMask::POST_ACTION | ProcMask::POST_DAMAGE
    );
}

#[test]
fn registry_resolves_skill_specs_by_local_name_and_export_name() {
    let mut builder = ExtensionRegistryBuilder::default();

    let custom_fire = builder
        .register_skill("custom", "fire", "custom.fire", TargetPolicy::Enemy, SkillPriority(10))
        .expect("custom fire should register");
    let core_fire = builder
        .register_skill("core", "fire", "core.fire", TargetPolicy::Enemy, SkillPriority(20))
        .expect("core fire should register");
    let disperse = builder
        .register_skill("core", "disperse", "core.disperse", TargetPolicy::Enemy, SkillPriority(30))
        .expect("disperse should register");

    let registry = builder.build();

    assert_eq!(registry.skill_by_name("custom", "fire").unwrap().id, custom_fire);
    assert_eq!(registry.skill_id_by_name("core", "fire"), Some(core_fire));
    assert_eq!(
        registry
            .skill_by_export_name("core.disperse")
            .map(|spec| (&spec.namespace, &spec.name, spec.id)),
        Some((&"core".to_owned(), &"disperse".to_owned(), disperse))
    );
    assert_eq!(registry.skill_id_by_export_name("custom.fire"), Some(custom_fire));
    assert_eq!(registry.skill_by_name("missing", "fire"), None);
    assert_eq!(registry.skill_by_export_name("legacy.fire"), None);
}

#[test]
fn registry_orders_skill_and_state_specs_by_priority_then_registration() {
    let mut builder = ExtensionRegistryBuilder::default();

    let late_skill = builder
        .register_skill("custom", "late", "custom.late", TargetPolicy::Enemy, SkillPriority(10))
        .expect("late skill should register");
    let early_skill = builder
        .register_skill("custom", "early", "custom.early", TargetPolicy::Enemy, SkillPriority(1))
        .expect("early skill should register");
    let tie_skill = builder
        .register_skill("custom", "tie", "custom.tie", TargetPolicy::Enemy, SkillPriority(10))
        .expect("tie skill should register");

    let late_state = builder
        .register_state(
            "custom",
            "late-state",
            "custom.late_state",
            ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("late state should register");
    let early_state = builder
        .register_state(
            "custom",
            "early-state",
            "custom.early_state",
            ProcMask::POST_ACTION,
            SkillPriority(1),
        )
        .expect("early state should register");
    let tie_state = builder
        .register_state(
            "custom",
            "tie-state",
            "custom.tie_state",
            ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("tie state should register");

    let registry = builder.build();

    assert_eq!(
        registry.skills_in_hook_order().into_iter().map(|spec| spec.id).collect::<Vec<_>>(),
        vec![early_skill, late_skill, tie_skill]
    );
    assert_eq!(
        registry.states_in_hook_order().into_iter().map(|spec| spec.id).collect::<Vec<_>>(),
        vec![early_state, late_state, tie_state]
    );
}

#[test]
fn registry_rejects_skill_and_state_name_and_export_collisions() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .register_skill("custom", "fire", "custom.fire", TargetPolicy::Enemy, SkillPriority(0))
        .expect("first skill should register");

    assert_eq!(
        builder.register_skill("custom", "fire", "custom.fire.v2", TargetPolicy::Enemy, SkillPriority(0)),
        Err(ExtensionError::DuplicateName {
            namespace: "custom".to_owned(),
            name: "fire".to_owned(),
        })
    );
    assert_eq!(
        builder.register_state("custom", "burning", "custom.fire", ProcMask::POST_ACTION, SkillPriority(0)),
        Err(ExtensionError::DuplicateExportName {
            export_name: "custom.fire".to_owned(),
        })
    );
}

#[test]
fn registry_stores_template_battle_and_entity_slot_specs() {
    let mut builder = ExtensionRegistryBuilder::default();

    let template = builder
        .reserve_template_slot("custom", "template-config", "custom.template_config")
        .expect("template slot should reserve");
    let battle = builder
        .reserve_battle_slot("custom", "battle-cache", "custom.battle_cache")
        .expect("battle slot should reserve");
    let entity = builder
        .reserve_entity_slot("custom", "entity-flags", "custom.entity_flags")
        .expect("entity slot should reserve");

    assert_eq!(template, TemplateSlotId(0));
    assert_eq!(battle, BattleSlotId(0));
    assert_eq!(entity, EntitySlotId(0));

    let registry = builder.build();
    assert_eq!(registry.template_slot(template).unwrap().name, "template-config");
    assert_eq!(registry.battle_slot(battle).unwrap().export_name, "custom.battle_cache");
    assert_eq!(registry.entity_slot(entity).unwrap().namespace, "custom");
}

#[test]
fn registry_rejects_slot_name_and_export_collisions() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .reserve_template_slot("custom", "config", "custom.config")
        .expect("template slot should reserve");

    assert_eq!(
        builder.reserve_template_slot("custom", "config", "custom.config.v2"),
        Err(ExtensionError::DuplicateName {
            namespace: "custom".to_owned(),
            name: "config".to_owned(),
        })
    );
    assert_eq!(
        builder.reserve_battle_slot("custom", "cache", "custom.config"),
        Err(ExtensionError::DuplicateExportName {
            export_name: "custom.config".to_owned(),
        })
    );
}

#[test]
fn registry_stores_effect_replay_and_show_renderer_specs() {
    let mut builder = ExtensionRegistryBuilder::default();

    let effect = builder
        .register_effect_handler("custom", "summon-damage", "custom.summon_damage", SkillPriority(10))
        .expect("effect handler should register");
    let replay = builder
        .register_replay_renderer("custom", "hp-marker", "custom.hp_marker.replay", SkillPriority(5))
        .expect("replay renderer should register");
    let show = builder
        .register_show_renderer("custom", "hp-marker", "custom.hp_marker.show", SkillPriority(5))
        .expect("show renderer should register");

    assert_eq!(effect, EffectHandlerId(0));
    assert_eq!(replay, ReplayRendererId(0));
    assert_eq!(show, ShowRendererId(0));

    let registry = builder.build();
    assert_eq!(registry.effect_handler(effect).unwrap().name, "summon-damage");
    assert_eq!(registry.replay_renderer(replay).unwrap().export_name, "custom.hp_marker.replay");
    assert_eq!(registry.show_renderer(show).unwrap().namespace, "custom");
}

#[test]
fn registry_orders_effect_and_renderer_specs_by_priority_then_registration() {
    let mut builder = ExtensionRegistryBuilder::default();

    let late_effect = builder
        .register_effect_handler("custom", "late-effect", "custom.late_effect", SkillPriority(10))
        .expect("late effect should register");
    let early_effect = builder
        .register_effect_handler("custom", "early-effect", "custom.early_effect", SkillPriority(1))
        .expect("early effect should register");
    let tie_effect = builder
        .register_effect_handler("custom", "tie-effect", "custom.tie_effect", SkillPriority(10))
        .expect("tie effect should register");

    let late_replay = builder
        .register_replay_renderer("custom", "late-replay", "custom.late_replay", SkillPriority(10))
        .expect("late replay should register");
    let early_replay = builder
        .register_replay_renderer("custom", "early-replay", "custom.early_replay", SkillPriority(1))
        .expect("early replay should register");
    let late_show = builder
        .register_show_renderer("custom", "late-show", "custom.late_show", SkillPriority(10))
        .expect("late show should register");
    let early_show = builder
        .register_show_renderer("custom", "early-show", "custom.early_show", SkillPriority(1))
        .expect("early show should register");

    let registry = builder.build();

    assert_eq!(
        registry
            .effect_handlers_in_chain_order()
            .into_iter()
            .map(|spec| spec.id)
            .collect::<Vec<_>>(),
        vec![early_effect, late_effect, tie_effect]
    );
    assert_eq!(
        registry.replay_renderers_in_order().into_iter().map(|spec| spec.id).collect::<Vec<_>>(),
        vec![early_replay, late_replay]
    );
    assert_eq!(
        registry.show_renderers_in_order().into_iter().map(|spec| spec.id).collect::<Vec<_>>(),
        vec![early_show, late_show]
    );
}

#[test]
fn registry_rejects_renderer_name_and_export_collisions() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .register_replay_renderer("custom", "hp", "custom.hp.replay", SkillPriority(0))
        .expect("replay renderer should register");

    assert_eq!(
        builder.register_replay_renderer("custom", "hp", "custom.hp.replay.v2", SkillPriority(0)),
        Err(ExtensionError::DuplicateName {
            namespace: "custom".to_owned(),
            name: "hp".to_owned(),
        })
    );
    assert_eq!(
        builder.register_show_renderer("custom", "hp", "custom.hp.replay", SkillPriority(0)),
        Err(ExtensionError::DuplicateExportName {
            export_name: "custom.hp.replay".to_owned(),
        })
    );
}

#[test]
fn registry_runs_extension_registration_flow() {
    let mut builder = ExtensionRegistryBuilder::default();

    builder.install_extension(&FixtureExtension).expect("fixture extension should install");

    let registry = builder.build();
    assert_eq!(
        registry.installed_extensions(),
        &[InstalledExtensionSpec {
            name: "fixture".to_owned(),
            version: ExtensionVersion::new(1, 2, 3),
            capabilities: vec![ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
        }]
    );
    assert_eq!(registry.player_kinds()[0].export_name, "fixture.kind");
    assert_eq!(registry.skills()[0].priority, SkillPriority(3));
    assert_eq!(registry.entity_slots()[0].name, "flags");
}

#[test]
fn registry_rejects_duplicate_extension_names() {
    let mut builder = ExtensionRegistryBuilder::default();

    builder.install_extension(&FixtureExtension).expect("first fixture should install");

    assert_eq!(
        builder.install_extension(&FixtureExtension),
        Err(ExtensionError::DuplicateExtensionName {
            name: "fixture".to_owned(),
        })
    );
}

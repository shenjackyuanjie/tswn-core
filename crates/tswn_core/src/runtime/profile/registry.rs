use super::*;

pub fn default_custom_runtime_import_config() -> Result<CustomRuntimeImportConfig<'static>, DefaultCustomRuntimeProfileError> {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder.register_skill(
        "custom",
        "summon",
        DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT,
        TargetPolicy::Enemy,
        SkillPriority(0),
    )?;
    let summon_fire = builder.register_skill(
        "custom",
        "summon-fire",
        DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT,
        TargetPolicy::Enemy,
        SkillPriority(1),
    )?;
    let summon_explode = builder.register_skill(
        "custom",
        "summon-explode",
        DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT,
        TargetPolicy::Enemy,
        SkillPriority(2),
    )?;
    let possess = builder.register_skill_with_hooks(
        "custom",
        "minion-possess",
        DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT,
        ProcMask::NONE,
        TargetPolicy::Enemy,
        SkillPriority(3),
    )?;
    builder.register_skill(
        "custom",
        "minion-heal",
        "custom.minion.heal",
        TargetPolicy::Ally,
        SkillPriority(4),
    )?;
    for builtin_skill in BuiltinActiveSkill::CORE {
        if builtin_skill == BuiltinActiveSkill::Charge {
            builder.register_skill_with_hooks_and_post_action_phase(
                "core",
                builtin_skill.local_name(),
                builtin_skill.export_name(),
                ProcMask::POST_ACTION,
                builtin_skill.target_policy(),
                SkillPriority(builtin_skill.legacy_key() as i32),
                SkillPostActionPhase::Late,
            )?
        } else {
            builder.register_skill_with_hooks(
                "core",
                builtin_skill.local_name(),
                builtin_skill.export_name(),
                ProcMask::NONE,
                builtin_skill.target_policy(),
                SkillPriority(builtin_skill.legacy_key() as i32),
            )?
        };
    }
    builder.register_skill_with_hooks(
        "core",
        "summon-explode",
        DEFAULT_CORE_SUMMON_EXPLODE_SKILL_EXPORT,
        ProcMask::NONE,
        TargetPolicy::Enemy,
        SkillPriority(2),
    )?;
    builder.register_skill_with_hooks(
        "core",
        "summon-share-damage",
        DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT,
        ProcMask::NONE,
        TargetPolicy::None,
        SkillPriority(255),
    )?;
    builder.register_state(
        "core",
        "charm",
        DEFAULT_CORE_CHARM_STATE_EXPORT,
        ProcMask::POST_ACTION,
        SkillPriority(210),
    )?;
    builder.register_state(
        "core",
        "curse",
        DEFAULT_CORE_CURSE_STATE_EXPORT,
        ProcMask::POST_DEFEND,
        SkillPriority(10_000),
    )?;
    builder.register_state(
        "core",
        "poison",
        DEFAULT_CORE_POISON_STATE_EXPORT,
        ProcMask::POST_ACTION,
        SkillPriority(150),
    )?;
    builder.register_state(
        "core",
        "haste",
        DEFAULT_CORE_HASTE_STATE_EXPORT,
        ProcMask::POST_ACTION,
        SkillPriority(210),
    )?;
    builder.register_state(
        "core",
        "slow",
        DEFAULT_CORE_SLOW_STATE_EXPORT,
        ProcMask::POST_ACTION,
        SkillPriority(210),
    )?;
    builder.register_state(
        "core",
        "iron",
        DEFAULT_CORE_IRON_STATE_EXPORT,
        ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
        SkillPriority(10),
    )?;
    builder.register_state(
        "core",
        "covid-infection",
        DEFAULT_CORE_COVID_INFECTION_STATE_EXPORT,
        ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
        SkillPriority(1000),
    )?;
    builder.register_state(
        "core",
        "lazy-infection",
        DEFAULT_CORE_LAZY_INFECTION_STATE_EXPORT,
        ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
        SkillPriority(1000),
    )?;
    builder.register_state(
        "core",
        "saitama-boss",
        DEFAULT_CORE_SAITAMA_BOSS_STATE_EXPORT,
        ProcMask::POST_DEFEND,
        SkillPriority(i32::MAX),
    )?;
    builder.register_player_kind_with_policies(
        "core",
        "boss",
        DEFAULT_CORE_BOSS_KIND_EXPORT,
        PlayerKindFlags::BOSS,
        PlayerKindPolicies::default(),
    )?;
    builder.register_player_kind_with_policies(
        "core",
        "boost",
        DEFAULT_CORE_BOOST_KIND_EXPORT,
        PlayerKindFlags::BOOST,
        PlayerKindPolicies::default(),
    )?;
    builder.register_player_kind_with_policies(
        "core",
        "shadow",
        DEFAULT_CORE_SHADOW_KIND_EXPORT,
        PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
        PlayerKindPolicies::default(),
    )?;
    builder.register_player_kind_with_policies(
        "core",
        "summon",
        DEFAULT_CORE_SUMMON_KIND_EXPORT,
        PlayerKindFlags::SUMMON | PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
        PlayerKindPolicies {
            merge: MergePolicy::FixedLane,
            ..PlayerKindPolicies::default()
        },
    )?;
    builder.register_player_kind_with_policies(
        "core",
        "zombie",
        DEFAULT_CORE_ZOMBIE_KIND_EXPORT,
        PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
        PlayerKindPolicies::default(),
    )?;
    builder.register_player_kind_with_policies(
        "core",
        "clone",
        DEFAULT_CORE_CLONE_KIND_EXPORT,
        PlayerKindFlags::MINION,
        PlayerKindPolicies::default(),
    )?;
    builder.register_skill_with_hooks(
        "core",
        "shield",
        DEFAULT_CORE_SHIELD_SKILL_EXPORT,
        ProcMask::PRE_ACTION,
        TargetPolicy::None,
        SkillPriority(0),
    )?;
    builder.register_skill_with_hooks(
        "core",
        "protect",
        DEFAULT_CORE_PROTECT_SKILL_EXPORT,
        ProcMask::POST_ACTION,
        TargetPolicy::Ally,
        SkillPriority(0),
    )?;
    builder.register_skill_with_hooks(
        "core",
        "defend",
        DEFAULT_CORE_DEFEND_SKILL_EXPORT,
        ProcMask::POST_DEFEND,
        TargetPolicy::None,
        SkillPriority(2000),
    )?;
    builder.register_skill_with_hooks(
        "core",
        "reflect",
        DEFAULT_CORE_REFLECT_SKILL_EXPORT,
        ProcMask::PRE_DEFEND,
        TargetPolicy::None,
        SkillPriority(1000),
    )?;
    builder.register_skill(
        "core",
        "upgrade",
        DEFAULT_CORE_UPGRADE_SKILL_EXPORT,
        TargetPolicy::None,
        SkillPriority(33),
    )?;
    builder.register_skill(
        "core",
        "hide",
        DEFAULT_CORE_HIDE_SKILL_EXPORT,
        TargetPolicy::None,
        SkillPriority(34),
    )?;
    builder.register_skill_with_hooks(
        "core",
        "counter",
        DEFAULT_CORE_COUNTER_SKILL_EXPORT,
        ProcMask::POST_DAMAGE,
        TargetPolicy::None,
        SkillPriority(30),
    )?;
    builder.register_skill_with_hooks(
        "core",
        "merge",
        DEFAULT_CORE_MERGE_SKILL_EXPORT,
        ProcMask::KILL,
        TargetPolicy::Enemy,
        SkillPriority(31),
    )?;
    builder.register_skill_with_hooks(
        "core",
        "zombie",
        DEFAULT_CORE_ZOMBIE_SKILL_EXPORT,
        ProcMask::KILL,
        TargetPolicy::Enemy,
        SkillPriority(32),
    )?;
    builder.register_skill_with_hooks(
        "core",
        "reraise",
        DEFAULT_CORE_RERAISE_SKILL_EXPORT,
        ProcMask::DIE,
        TargetPolicy::None,
        SkillPriority(10),
    )?;
    builder.reserve_entity_slot("core", "shadow-blueprint", DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT)?;
    builder.reserve_entity_slot("core", "summon-blueprint", DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT)?;
    builder.reserve_entity_slot("core", "zombie-blueprint", DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT)?;
    builder.reserve_entity_slot("core", "lazy-blueprint-rq", DEFAULT_CORE_LAZY_BLUEPRINT_RQ_ENTITY_EXPORT)?;
    builder.reserve_entity_slot("core", "summoned-entity", DEFAULT_CORE_SUMMON_ENTITY_EXPORT)?;
    builder.reserve_entity_slot("core", "minion-counter", DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT)?;
    builder.reserve_entity_slot("custom", "bed2-summoned-entity", DEFAULT_CUSTOM_BED2_SUMMON_ENTITY_EXPORT)?;
    let summon_template_slot =
        builder.reserve_template_slot("custom", "bed2-summon-template", DEFAULT_CUSTOM_BED2_SUMMON_TEMPLATE_EXPORT)?;
    let shadow_template_slot =
        builder.reserve_template_slot("custom", "bed2-shadow-template", DEFAULT_CUSTOM_BED2_SHADOW_TEMPLATE_EXPORT)?;
    let zombie_template_slot =
        builder.reserve_template_slot("custom", "bed2-zombie-template", DEFAULT_CUSTOM_BED2_ZOMBIE_TEMPLATE_EXPORT)?;
    let bed2 = builder.register_player_kind_with_policies(
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
    )?;
    let summon_kind = builder.register_player_kind_with_policies(
        "custom",
        "bed2-summon",
        DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT,
        PlayerKindFlags::SUMMON | PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
        PlayerKindPolicies {
            owner_resolution: OwnerResolutionPolicy::RootOwner,
            damage_share: DamageSharePolicy::ShareToOwner,
            merge: MergePolicy::FixedLane,
            inherit_owner_def_res: true,
        },
    )?;
    let shadow_kind = builder.register_player_kind_with_policies(
        "custom",
        "bed2-shadow",
        DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT,
        PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
        PlayerKindPolicies {
            owner_resolution: OwnerResolutionPolicy::RootOwner,
            damage_share: DamageSharePolicy::ShareToOwner,
            merge: MergePolicy::FixedLane,
            inherit_owner_def_res: false,
        },
    )?;
    let zombie_kind = builder.register_player_kind_with_policies(
        "custom",
        "bed2-zombie",
        DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT,
        PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
        PlayerKindPolicies {
            owner_resolution: OwnerResolutionPolicy::RootOwner,
            damage_share: DamageSharePolicy::ShareToOwner,
            merge: MergePolicy::FixedLane,
            inherit_owner_def_res: false,
        },
    )?;
    Ok(CustomRuntimeImportConfig::new(builder.build(), bed2, summon)
        .with_bed2_minion_overlays(CustomBed2MinionOverlayConfig {
            summon: CustomBed2SummonTemplateConfig {
                template_slot: summon_template_slot,
                summon_kind,
                fire_skill_export_name: DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT,
                explode_skill_export_name: DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT,
            },
            shadow: CustomBed2ShadowTemplateConfig {
                template_slot: shadow_template_slot,
                shadow_kind,
                possess_skill_export_name: DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT,
            },
            zombie: CustomBed2ZombieTemplateConfig {
                template_slot: zombie_template_slot,
                zombie_kind,
                skill_export_name_prefix: DEFAULT_CUSTOM_MINION_SKILL_EXPORT_PREFIX,
            },
        })
        .with_skill_handler_with_capabilities(
            summon,
            run_summon_recast_from_template_slot,
            &[
                ExtensionCapability::ReadTemplateSlots,
                ExtensionCapability::ReadAllies,
                ExtensionCapability::MutateEntitySlots,
            ],
        )
        .with_skill_handler(summon_fire, run_summon_fire_skill)
        .with_skill_handler(summon_explode, run_summon_explode_skill)
        .with_skill_handler(possess, run_possess_skill))
}

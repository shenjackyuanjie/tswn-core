//! 召唤（summon）技能与分担伤害：生成/复活、资源减半与 owner 死亡清理。
use super::*;

/// 用自定义混合输入构建召唤场景的 CombatRuntime。
pub fn summon_runtime() -> CombatRuntime {
    let raw = "Stupefy #rkISERW8@Shabby_fish\n日落·日出 #Pd3J7shds@Shabby_fish";
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    RuntimeRunner::from_custom_mixed_namerena_raw(raw.to_owned(), config)
        .expect("runtime summon runner should build")
        .runtime
}

/// 读取召唤体上 share-damage 技能 lane 的等级。
fn summon_share_level(runtime: &CombatRuntime, summoned: EntityIdx) -> u32 {
    let skills = &runtime.entities.get(summoned).unwrap().template.skills;
    let lane = (0..skills.len())
        .find(|lane| skills.fixed_lane_key_at(*lane) == Some(crate::namerena::SUMMON_SHARE_DAMAGE_SKILL_KEY))
        .expect("summon should contain share-damage lane");
    skills.level_at(lane).unwrap()
}

#[test]
fn default_custom_profile_halves_bed2_summon_merge_resources() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let summon_kind = config
        .bed2_minion_overlays
        .as_ref()
        .expect("default runtime profile should configure bed2 minion overlays")
        .summon
        .summon_kind;

    assert_eq!(
        config.registry.player_kind(summon_kind).unwrap().policies.merge,
        MergePolicy::FixedLaneHalfResources
    );
}
#[test]
fn plain_summon_probability_gates_low_smart_hp_and_alive_remembered_entity_without_rng() {
    let mut runtime = summon_runtime();
    let owner = EntityIdx(1);
    runtime.entities.get_mut(owner).unwrap().runtime.hp = 79;
    let expected_rng = runtime.rng.clone();
    assert!(!runtime.plain_action_skill_probability(owner, BuiltinActiveSkill::Summon, 128, true));
    assert_rng_state_eq(&runtime.rng, &expected_rng);

    runtime.entities.get_mut(owner).unwrap().runtime.hp = 100;
    let mut updates = RunUpdates::new();
    runtime.drain_plain_summon_skill_into(owner, &mut updates);
    let expected_rng = runtime.rng.clone();
    assert!(!runtime.plain_action_skill_probability(owner, BuiltinActiveSkill::Summon, 128, false));
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn plain_summon_spawns_once_then_resets_and_revives_the_same_entity() {
    let mut runtime = summon_runtime();
    let owner = EntityIdx(1);
    let summoned = EntityIdx(3);
    let mut expected_rng = runtime.rng.clone();
    let expected_move_points = expected_rng.r255() as i32 * 4;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_skill_into(owner, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.len(), 4);
    let summoned_entity = runtime.entities.get(summoned).expect("summon should spawn");
    assert_eq!(summoned_entity.runtime.owner, owner);
    assert_eq!(summoned_entity.runtime.move_state.speed_points, expected_move_points);
    assert!(summoned_entity.runtime.flags.contains(PlayerKindFlags::SUMMON));
    assert_eq!(summon_share_level(&runtime, summoned), 1);
    let remembered_slot = runtime.registry.entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_ENTITY_EXPORT).unwrap();
    assert_eq!(
        runtime.entities.get(owner).unwrap().slots.get(remembered_slot),
        Some(&SlotValue::U64(u64::from(summoned.0)))
    );
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]使用[血祭]", "召唤出[1]"]
    );

    let original_name = summoned_entity.template.name.clone();
    let original_defense = summoned_entity.template.defense;
    let original_resistance = summoned_entity.template.resistance;
    let next_boost_lane = summoned_entity
        .template
        .skills
        .active_order()
        .iter()
        .rev()
        .copied()
        .find(|lane| {
            summoned_entity.template.skills.fixed_lane_key_at(*lane) != Some(crate::namerena::SUMMON_SHARE_DAMAGE_SKILL_KEY)
                && summoned_entity.template.skills.level_at(*lane).is_some_and(|level| level > 0)
                && summoned_entity.template.skills.boosted_at(*lane) == Some(false)
        })
        .expect("fixture summon should retain an unboosted active skill for recast");
    let next_boost_base = summoned_entity.template.skills.level_at(next_boost_lane).unwrap();
    {
        let owner_entity = runtime.entities.get_mut(owner).unwrap();
        let stats = {
            let build = owner_entity
                .template
                .clone_build
                .as_mut()
                .expect("summon owner should retain clone build data");
            build.decay_owner();
            build.derive_stats()
        };
        owner_entity.apply_derived_stats(stats);
    }
    assert_ne!(runtime.entities.get(owner).unwrap().template.defense, original_defense);
    {
        let summoned_entity = runtime.entities.get_mut(summoned).unwrap();
        summoned_entity.runtime.hp = 0;
        summoned_entity.runtime.alive = false;
        summoned_entity.states.add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 2));
        let team = summoned_entity.runtime.team;
        runtime.world.mark_dead(summoned, team);
    }
    runtime.entities.get_mut(owner).unwrap().runtime.at_boost_millionths = 3_000_000;
    let mut expected_rng = runtime.rng.clone();
    let _ = expected_rng.r255();
    let mut recast_updates = RunUpdates::new();

    runtime.drain_plain_summon_skill_into(owner, &mut recast_updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.len(), 4);
    let revived = runtime.entities.get(summoned).unwrap();
    assert!(revived.runtime.alive);
    assert_eq!(revived.template.name, original_name);
    assert_eq!(revived.template.defense, original_defense);
    assert_eq!(revived.template.resistance, original_resistance);
    assert_eq!(revived.runtime.move_state.speed_points, 2048);
    assert!(revived.states.entries().is_empty());
    assert_eq!(summon_share_level(&runtime, summoned), 0);
    assert_eq!(revived.template.skills.level_at(next_boost_lane), Some(next_boost_base * 2));
    assert_eq!(revived.template.skills.boosted_at(next_boost_lane), Some(true));
    assert_eq!(
        revived.template.skills.boost_at(next_boost_lane),
        Some(&crate::namerena::SkillBoost::LastBoost(next_boost_base))
    );
    assert_eq!(
        recast_updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]使用[血祭]", "召唤出[1]"]
    );
}

#[test]
fn summon_explode_self_death_suppresses_disappear_after_last_enemy_dies() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "core",
            "summon",
            "core.summon",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION | PlayerKindFlags::SUMMON,
            PlayerKindPolicies::default(),
        )
        .expect("summon kind should register");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 100, 1),
            PlayerTemplate::with_kind(2, "owner?0", summon_kind, 0, 10, 1),
            PlayerTemplate::new(3, "last-enemy", 1, 10, 1),
        ],
        builder.build(),
    ));
    let summon = EntityIdx(1);
    let enemy = EntityIdx(2);
    runtime.entities.get_mut(summon).unwrap().runtime.hp = 0;
    runtime.entities.get_mut(enemy).unwrap().runtime.hp = 0;
    runtime.entities.get_mut(enemy).unwrap().runtime.alive = false;
    runtime.world.mark_dead(enemy, 1);
    let mut updates = RunUpdates::new();

    runtime.drain_summon_explode_self_death_into(summon, &mut updates);

    assert!(!runtime.entities.get(summon).unwrap().runtime.alive);
    assert!(updates.updates.is_empty());
}

#[test]
fn plain_summon_share_damage_halves_damage_and_does_not_cleanup_source_when_owner_dies() {
    let mut runtime = summon_runtime();
    let owner = EntityIdx(1);
    let caster = EntityIdx(0);
    let summoned = EntityIdx(3);
    runtime.drain_plain_summon_skill_into(owner, &mut RunUpdates::new());
    runtime.entities.get_mut(owner).unwrap().runtime.hp = 5;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_share_damage_into(summoned, 1, 11, caster, &mut updates);

    assert_eq!(runtime.entities.get(owner).unwrap().runtime.hp, 0);
    assert!(!runtime.entities.get(owner).unwrap().runtime.alive);
    assert!(runtime.entities.get(summoned).unwrap().runtime.alive);
    assert_eq!(updates.updates.first().unwrap().score, 5);
}

#[test]
fn plain_summon_explode_uses_static_effect_path_and_kills_the_summon() {
    let mut runtime = summon_runtime();
    let owner = EntityIdx(1);
    let target = EntityIdx(0);
    let summoned = EntityIdx(3);
    runtime.drain_plain_summon_skill_into(owner, &mut RunUpdates::new());
    let target_hp = runtime.entities.get(target).unwrap().runtime.hp;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_explode_into(summoned, target, &mut updates);

    assert!(!runtime.entities.get(summoned).unwrap().runtime.alive);
    assert!(runtime.entities.get(target).unwrap().runtime.hp < target_hp);
    assert_eq!(updates.updates.first().unwrap().message, "[0]使用[自爆]");
}

#[test]
fn summon_explode_effect_emits_legacy_replay_and_kills_summon() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "owner", 0, 10, 3),
        PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
        PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
    ]));
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::fire_mag(91, 3));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 5.5;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let expected_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 2.0);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[0].caster, 2);
    assert_eq!(frame.updates.updates[0].target, 1);
    assert_eq!(frame.updates.updates[0].score, 0);
    let expected_damage_update = RuntimeFrame::legacy_damage_update(2, 1, expected_amount);
    assert_eq!(frame.updates.updates[1].message, expected_damage_update.message);
    assert_eq!(frame.updates.updates[1].caster, 2);
    assert_eq!(frame.updates.updates[1].target, 1);
    assert_eq!(frame.updates.updates[1].score, expected_amount as u32);
    assert_eq!(frame.updates.updates[1].delay0, expected_damage_update.delay0);
}

#[test]
fn summon_share_damage_owner_death_marks_active_summon_for_outer_lethal_chain() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 1),
            PlayerTemplate::new(2, "caster", 1, 100, 1),
        ],
        config.registry,
    ));
    let summoned = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?0", summon_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(0)),
    );
    let summon_team = runtime.entities.get(summoned).unwrap().runtime.team;
    runtime.world.add_spawned_alive(summoned, summon_team);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_share_damage_into(summoned, 1, 50, EntityIdx(1), &mut updates);

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(owner.runtime.hp, 0);
    assert!(!owner.runtime.alive);
    let summoned_entity = runtime.entities.get(summoned).unwrap();
    assert_eq!(summoned_entity.runtime.hp, 0);
    assert!(summoned_entity.runtime.alive);
    assert!(runtime.world.round_order().contains(&summoned));
    assert!(runtime.world.flat_alive().contains(&summoned));
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[1]受到[2]点伤害", "\n", "[1]被击倒了"]
    );
    assert_eq!(updates.updates[2].caster, 1);
    assert_eq!(updates.updates[2].target, 0);
    assert_eq!(updates.updates[2].score, 50);
}

#[test]
fn half_skill_finishes_active_summon_lethal_chain_after_owner_share_death() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let summon_share = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT)
        .expect("default profile should register summon share damage");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 1),
            PlayerTemplate::new(2, "caster", 1, 100, 1).with_magic(100),
        ],
        config.registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().activate_charge_runtime();
    let summoned = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?0", summon_kind, 0, 100, 0)
            .with_skill_loadout(SkillLoadout::from_skill_levels([(summon_share, 1)])),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(0)),
    );
    let summon_team = runtime.entities.get(summoned).unwrap().runtime.team;
    runtime.world.add_spawned_alive(summoned, summon_team);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_half_skill_into(EntityIdx(1), summoned, &mut updates);

    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    let summoned_entity = runtime.entities.get(summoned).unwrap();
    assert_eq!(summoned_entity.runtime.hp, 0);
    assert!(!summoned_entity.runtime.alive);
    assert!(!runtime.world.round_order().contains(&summoned));
    assert!(!runtime.world.flat_alive().contains(&summoned));
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert!(
        updates
            .updates
            .iter()
            .any(|update| update.target == summoned.0 as usize && update.message == "[1]消失了")
    );
}

#[test]
fn summon_share_damage_hide_counts_zero_hp_summon_before_outer_lethal_chain() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let hide = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_HIDE_SKILL_EXPORT)
        .expect("default profile should register hide skill");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 100, 1).with_skill_loadout(SkillLoadout::from_skill_levels([(hide, 20)])),
            PlayerTemplate::new(2, "caster", 1, 100, 1),
        ],
        config.registry,
    ));
    let summoned = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?0", summon_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(0)),
    );
    let summon_team = runtime.entities.get(summoned).unwrap().runtime.team;
    runtime.world.add_spawned_alive(summoned, summon_team);
    runtime.entities.get_mut(summoned).unwrap().runtime.hp = 0;
    let mut expected_rng = runtime.rng.clone();
    expected_rng.r63();
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_share_damage_into(summoned, 1, 10, EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn summon_share_damage_owner_death_removes_root_owned_sibling_minion_when_direct_owner_is_dead() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "root", 0, 100, 1),
            PlayerTemplate::new(2, "owner", 0, 20, 1),
            PlayerTemplate::new(3, "caster", 1, 100, 1),
        ],
        config.registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.owner = EntityIdx(0);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.root_owner = EntityIdx(0);
    let active_summon = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?0", summon_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(1)),
        Some(EntityIdx(0)),
    );
    let sibling = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "root?0", summon_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(1)),
    );
    let active_team = runtime.entities.get(active_summon).unwrap().runtime.team;
    let sibling_team = runtime.entities.get(sibling).unwrap().runtime.team;
    runtime.world.add_spawned_alive(active_summon, active_team);
    runtime.world.add_spawned_alive(sibling, sibling_team);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 0;
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;
    runtime.world.mark_dead(EntityIdx(0), 0);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_share_damage_into(active_summon, 1, 50, EntityIdx(2), &mut updates);

    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    let active_summon_entity = runtime.entities.get(active_summon).unwrap();
    assert_eq!(active_summon_entity.runtime.hp, 0);
    assert!(active_summon_entity.runtime.alive);
    assert!(!runtime.entities.get(sibling).unwrap().runtime.alive);
    assert!(runtime.world.round_order().contains(&active_summon));
    assert!(!runtime.world.round_order().contains(&sibling));
    assert!(runtime.world.flat_alive().contains(&active_summon));
    assert!(!runtime.world.flat_alive().contains(&sibling));
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[1]受到[2]点伤害", "\n", "[1]被击倒了", "\n", "[1]消失了"]
    );
    assert_eq!(updates.updates[4].caster, 1);
    assert_eq!(updates.updates[4].target, sibling.0 as usize);
    assert_eq!(updates.updates[4].score, 50);
}

#[test]
fn summon_share_damage_owner_death_removes_direct_shadow_but_keeps_root_owned_shadow() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let shadow_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SHADOW_KIND_EXPORT)
        .expect("default profile should register core shadow kind");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "root", 0, 100, 1),
            PlayerTemplate::new(2, "owner", 0, 20, 1),
            PlayerTemplate::new(3, "caster", 1, 100, 1),
        ],
        config.registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.owner = EntityIdx(0);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.root_owner = EntityIdx(0);
    let active_summon = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?0", summon_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(1)),
        Some(EntityIdx(0)),
    );
    let sibling_shadow = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "root?shadow", shadow_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(1)),
    );
    let direct_shadow = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?shadow", shadow_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(1)),
        Some(EntityIdx(0)),
    );
    let active_team = runtime.entities.get(active_summon).unwrap().runtime.team;
    let shadow_team = runtime.entities.get(sibling_shadow).unwrap().runtime.team;
    let direct_shadow_team = runtime.entities.get(direct_shadow).unwrap().runtime.team;
    runtime.world.add_spawned_alive(active_summon, active_team);
    runtime.world.add_spawned_alive(sibling_shadow, shadow_team);
    runtime.world.add_spawned_alive(direct_shadow, direct_shadow_team);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_share_damage_into(active_summon, 1, 50, EntityIdx(2), &mut updates);

    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    assert!(runtime.entities.get(sibling_shadow).unwrap().runtime.alive);
    assert!(runtime.world.round_order().contains(&sibling_shadow));
    assert!(runtime.world.flat_alive().contains(&sibling_shadow));
    assert!(!runtime.entities.get(direct_shadow).unwrap().runtime.alive);
    assert!(!runtime.world.round_order().contains(&direct_shadow));
    assert!(!runtime.world.flat_alive().contains(&direct_shadow));
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[1]受到[2]点伤害", "\n", "[1]被击倒了", "\n", "[1]消失了"]
    );
    assert_eq!(updates.updates[4].target, direct_shadow.0 as usize);
}

#[test]
fn summon_share_damage_reraise_keeps_owner_in_round_order() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let reraise = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_RERAISE_SKILL_EXPORT)
        .expect("default profile should register reraise");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 1).with_skill_loadout(SkillLoadout::from_skill_levels([(reraise, 128)])),
            PlayerTemplate::new(2, "caster", 1, 100, 1),
            PlayerTemplate::new(3, "tail", 1, 100, 1),
        ],
        config.registry,
    ));
    runtime.set_skill_handler(reraise, run_reraise_die_skill);
    let summoned = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?0", summon_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(0)),
    );
    let summon_team = runtime.entities.get(summoned).unwrap().runtime.team;
    runtime.world.add_spawned_alive(summoned, summon_team);
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(0)));
    let before_order = runtime.world.round_order().to_vec();
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_share_damage_into(summoned, 1, 50, EntityIdx(1), &mut updates);

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(owner.runtime.alive);
    assert!(owner.runtime.hp > 0);
    assert_eq!(runtime.world.round_order(), before_order.as_slice());
    assert!(runtime.entities.get(summoned).unwrap().runtime.alive);
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(1)));
}

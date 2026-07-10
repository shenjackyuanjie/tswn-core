use super::*;

fn summon_runtime() -> CombatRuntime {
    let raw = "Stupefy #rkISERW8@Shabby_fish\n日落·日出 #Pd3J7shds@Shabby_fish";
    let config = default_custom_runtime_v2_import_config().expect("default runtime v2 profile should build");
    RuntimeV2Runner::from_custom_mixed_namerena_raw(raw.to_owned(), config)
        .expect("runtime v2 summon runner should build")
        .runtime
}

fn summon_share_level(runtime: &CombatRuntime, summoned: EntityIdx) -> u32 {
    let skills = &runtime.entities.get(summoned).unwrap().template.skills;
    let lane = (0..skills.len())
        .find(|lane| skills.fixed_lane_key_at(*lane) == Some(crate::player::skill::act::summon::SUMMON_SHARE_DAMAGE_SKILL_KEY))
        .expect("summon should contain share-damage lane");
    skills.level_at(lane).unwrap()
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
    let summoned = EntityIdx(2);
    let mut expected_rng = runtime.rng.clone();
    let expected_move_points = expected_rng.r255() as i32 * 4;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_skill_into(owner, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.len(), 3);
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
    assert_eq!(runtime.entities.len(), 3);
    let revived = runtime.entities.get(summoned).unwrap();
    assert!(revived.runtime.alive);
    assert_eq!(revived.template.name, original_name);
    assert_eq!(revived.runtime.move_state.speed_points, 2048);
    assert!(revived.states.entries().is_empty());
    assert_eq!(summon_share_level(&runtime, summoned), 0);
    assert_eq!(
        recast_updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]使用[血祭]", "召唤出[1]"]
    );
}

#[test]
fn plain_clone_inherits_summon_blueprint_and_can_summon() {
    let mut runtime = summon_runtime();
    let owner = EntityIdx(1);
    let clone_lane = {
        let skills = &runtime.entities.get(owner).unwrap().template.skills;
        (0..skills.len())
            .find(|lane| skills.fixed_lane_key_at(*lane) == Some(BuiltinActiveSkill::Clone.legacy_key()))
            .expect("fixture owner should contain clone")
    };

    runtime.drain_plain_clone_skill_into(owner, clone_lane, &mut RunUpdates::new());

    let clone = EntityIdx(2);
    let blueprint_slot = runtime
        .registry
        .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT)
        .expect("core summon blueprint slot should exist");
    assert!(matches!(
        runtime.entities.get(clone).unwrap().slots.get(blueprint_slot),
        Some(SlotValue::PlayerTemplate(_))
    ));

    let mut updates = RunUpdates::new();
    runtime.drain_plain_summon_skill_into(clone, &mut updates);

    let summoned = EntityIdx(3);
    assert_eq!(runtime.entities.get(summoned).unwrap().runtime.owner, clone);
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]使用[血祭]", "召唤出[1]"]
    );
}

#[test]
fn plain_summon_share_damage_halves_damage_and_does_not_cleanup_source_when_owner_dies() {
    let mut runtime = summon_runtime();
    let owner = EntityIdx(1);
    let caster = EntityIdx(0);
    let summoned = EntityIdx(2);
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
    let summoned = EntityIdx(2);
    runtime.drain_plain_summon_skill_into(owner, &mut RunUpdates::new());
    let target_hp = runtime.entities.get(target).unwrap().runtime.hp;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_explode_into(summoned, target, &mut updates);

    assert!(!runtime.entities.get(summoned).unwrap().runtime.alive);
    assert!(runtime.entities.get(target).unwrap().runtime.hp < target_hp);
    assert_eq!(updates.updates.first().unwrap().message, "[0]使用[自爆]");
}

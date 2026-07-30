use super::*;

#[test]
fn plain_clone_inherits_summon_blueprint_and_can_summon() {
    let mut runtime = super::plain_summon_skill_tests::summon_runtime();
    let owner = EntityIdx(1);
    let clone_lane = {
        let skills = &runtime.entities.get(owner).unwrap().template.skills;
        (0..skills.len())
            .find(|lane| skills.fixed_lane_key_at(*lane) == Some(BuiltinActiveSkill::Clone.legacy_key()))
            .expect("fixture owner should contain clone")
    };

    runtime.drain_plain_clone_skill_into(owner, clone_lane, &mut RunUpdates::new());

    let clone = EntityIdx(2);
    let clone_entity = runtime.entities.get(clone).expect("clone should spawn");
    assert!(clone_entity.runtime.is_minion());
    assert!(!clone_entity.runtime.is_combat_minion());
    assert_eq!(
        clone_entity.runtime.kind,
        runtime
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_CLONE_KIND_EXPORT)
            .expect("core clone kind should exist")
    );
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

    let summoned = EntityIdx(4);
    assert_eq!(runtime.entities.get(summoned).unwrap().runtime.owner, clone);
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]使用[血祭]", "召唤出[1]"]
    );

    let mut clone_death_updates = RunUpdates::new();
    runtime.emit_plain_lethal_replay_into(owner, clone, &mut clone_death_updates);
    assert_eq!(clone_death_updates.updates.last().unwrap().message, "[1]被击倒了");

    let mut summon_death_updates = RunUpdates::new();
    runtime.emit_plain_lethal_replay_into(owner, summoned, &mut summon_death_updates);
    assert_eq!(summon_death_updates.updates.last().unwrap().message, "[1]消失了");
}

#[test]
fn namer_pf_score_clone_rebuilds_reraise_level_like_legacy() {
    let raw = "! #NHe2ywg@Unbound\n33555277@!\n\n33555278@!\n33555279@!";
    let config = default_custom_runtime_import_config().expect("runtime config should build");
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw_with_eval_rq(
        raw.to_owned(),
        crate::namerena::eval_name::WIN_RATE_EVAL_RQ,
        config,
    )
    .expect("namer-pf round should build");

    for _ in 0..14 {
        runner.run_round();
    }

    let clone = runner
        .runtime()
        .entities
        .get(EntityIdx(4))
        .expect("round 14 should spawn the score profile clone");
    let reraise_lane = (0..clone.template.skills.len())
        .find(|lane| clone.template.skills.fixed_lane_key_at(*lane) == Some(28))
        .expect("score profile clone should contain reraise");

    assert_eq!(clone.template.skills.level_at(reraise_lane), Some(14));
}

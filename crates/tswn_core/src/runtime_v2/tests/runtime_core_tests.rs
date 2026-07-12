use super::*;

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

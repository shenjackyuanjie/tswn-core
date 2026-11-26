//! runtime 核心：模板构建、校验、trace、实体 ID 空洞与 prepared 初始化。
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
        RuntimeReadyError {
            missing_skill_handlers: vec![RuntimeMissingSkillHandler {
                skill_id: skill,
                export_name: Some("custom.missing".to_owned()),
                sources: vec![
                    RuntimeSkillSource::Entity(EntityIdx(0)),
                    RuntimeSkillSource::TemplateSlot(template_slot),
                ],
            }],
        }
    );
    assert_eq!(
        error.to_string(),
        "runtime missing skill handlers: custom.missing (id 0) used by entity 0, template slot 0"
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
fn entity_arena_preserves_reserved_player_id_holes() {
    let registry = ExtensionRegistry::default();
    let mut arena = EntityArena::from_templates_with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 1),
            PlayerTemplate::new(2, "right", 1, 10, 1),
        ],
        &registry,
    );
    let template = PlayerTemplate::new(0, "summon", 0, 5, 1).with_reserved_player_ids_before_spawn(1);

    let spawned = arena.spawn_from_template_with_owner(template, &registry, Some(EntityIdx(0)), Some(EntityIdx(0)));

    assert_eq!(spawned, EntityIdx(3));
    assert_eq!(arena.len(), 4);
    assert!(arena.get(EntityIdx(2)).is_none());
    assert_eq!(arena.get(spawned).unwrap().template.id, 4);
    assert_eq!(
        arena.iter().map(|(idx, entity)| (idx, entity.template.id)).collect::<Vec<_>>(),
        vec![(EntityIdx(0), 1), (EntityIdx(1), 2), (EntityIdx(3), 4)]
    );
}

#[test]
fn entity_arena_never_reuses_reserved_player_id_holes() {
    let registry = ExtensionRegistry::default();
    let mut arena = EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "owner", 0, 10, 1)], &registry);
    let reserved = PlayerTemplate::new(0, "summon", 0, 5, 1).with_reserved_player_ids_before_spawn(1);

    let summon = arena.spawn_from_template_with_owner(reserved, &registry, Some(EntityIdx(0)), Some(EntityIdx(0)));
    let clone = arena.spawn_from_template_with_owner(
        PlayerTemplate::new(0, "clone", 0, 5, 1),
        &registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(0)),
    );

    assert_eq!(summon, EntityIdx(2));
    assert_eq!(clone, EntityIdx(3));
    assert!(arena.get(EntityIdx(1)).is_none());
    assert_eq!(arena.get(summon).unwrap().template.id, 3);
    assert_eq!(arena.get(clone).unwrap().template.id, 4);
}

/// 把 namerena raw 输入经 build 构造成 runner，再套用 prepared battle init。
pub fn runtime_runner_from_raw<E: std::fmt::Debug>(
    raw_input: &str,
    build: impl FnOnce(&[Vec<String>]) -> Result<RuntimeRunner, E>,
) -> RuntimeRunner {
    let (raw_groups, seed) = PreparedBattleInit::split_namerena_raw(raw_input.to_owned());
    let mut runner = build(&raw_groups).expect("runtime roster builder should succeed");
    PreparedBattleInit::from_groups(&raw_groups, &seed, &runner.runtime.registry)
        .expect("runtime battle init should prepare")
        .apply(&mut runner.runtime)
        .expect("runtime battle init should apply");
    runner
}

#[test]
fn prepared_battle_init_split_keeps_group_and_seed_rules() {
    assert_eq!(
        PreparedBattleInit::split_namerena_raw("left@red\nright@blue".to_owned()),
        (vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]], Vec::new())
    );
    assert_eq!(
        PreparedBattleInit::split_namerena_raw("left@red\r\n\r\nseed:abc@!\r\n\r\nright@blue\r\n".to_owned()),
        (
            vec![
                vec!["left@red".to_owned(), "seed:abc@!".to_owned()],
                vec!["right@blue".to_owned()]
            ],
            vec!["seed:abc@!".to_owned()]
        )
    );
}

#[test]
fn prepared_battle_init_reports_entity_count_mismatch() {
    let raw_groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
    let template = PreparedCombatTemplate::new(vec![PlayerTemplate::new(1, "left", 0, 10, 3)]);
    let mut runtime = CombatRuntime::from_template(template);
    let error = PreparedBattleInit::from_groups(&raw_groups, &[], &runtime.registry)
        .expect("battle init should prepare")
        .apply(&mut runtime)
        .expect_err("battle init should reject a mismatched runtime");

    assert_eq!(error, RuntimeBattleInitError::EntityCountMismatch { prepared: 2, runtime: 1 });
}

#[test]
fn prepared_runtime_runner_reuses_roster_across_seeds_at_win_rate_eval_rq() {
    let raw_groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(
        &raw_groups,
        crate::namerena::eval_name::WIN_RATE_EVAL_RQ,
        config,
    )
    .expect("runtime roster should prepare");

    for seed in [
        Vec::new(),
        vec!["seed:33554432@!".to_owned()],
        vec!["seed:33554433@!".to_owned()],
        vec!["seed:prepared-runtime@!".to_owned()],
    ] {
        let mut first = prepared.new_with_seed(&seed).expect("runtime runner should instantiate");
        let mut second = prepared.new_with_seed(&seed).expect("runtime runner should instantiate twice");
        assert_eq!(
            first.run_until_winner_normalized_rounds(100_000),
            second.run_until_winner_normalized_rounds(100_000),
            "seed={seed:?}"
        );
    }
}

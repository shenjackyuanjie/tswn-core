use super::*;

fn runner() -> RuntimeRunner {
    RuntimeRunner::new_from_namerena_raw("private-left@red\n\nprivate-right@blue\nseed:private-seed".into()).unwrap()
}

#[test]
fn snapshot_is_read_only_and_excludes_identity_and_allocator_counters() {
    let mut runner = runner();
    runner.runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.counter.last_updates_id = Some(100);
    let before = runner.runtime.clone();
    let state = runner.model_state().unwrap();
    assert_eq!(before.entities, runner.runtime.entities);
    assert_eq!(before.world, runner.runtime.world);
    assert_eq!(before.rng.main_val, runner.runtime.rng.main_val);
    assert_eq!((before.rng.i, before.rng.j), (runner.runtime.rng.i, runner.runtime.rng.j));
    runner.runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.counter.last_updates_id = Some(999_999);
    assert_eq!(state, runner.model_state().unwrap());
    let json = serde_json::to_string(&state).unwrap();
    for forbidden in [
        "private-left",
        "private-right",
        "private-seed",
        "last_updates_id",
        "baseline_id",
        "clan_name",
        "main_val",
    ] {
        assert!(!json.contains(forbidden), "特征不应包含 {forbidden}");
    }
    assert_eq!(serde_json::from_str::<BattleModelState>(&json).unwrap(), state);
}

#[test]
fn skills_use_stable_one_based_registry_ids() {
    let registry = default_custom_runtime_import_config().unwrap().registry;
    assert_eq!(registry.skills().len(), 42);
    assert_eq!(
        registry.skills().iter().map(|skill| skill.export_name.as_str()).collect::<Vec<_>>(),
        MODEL_SKILL_EXPORTS
    );
    assert!(registry.skills().len() <= MODEL_SKILL_ID_LIMIT as usize);
    let loadout = SkillLoadout::from_skills(registry.skills().iter().map(|skill| skill.id));
    let skills = ModelSkills::from(&loadout);
    assert_eq!(
        skills.lanes.iter().map(|skill| skill.skill_id).collect::<Vec<_>>(),
        (1..=42).collect::<Vec<_>>()
    );
    assert_eq!(registry.skills()[0].export_name, DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT);
}

#[test]
fn preserves_hidden_payloads_registration_order_and_compressed_states() {
    let mut runner = runner();
    let states = &mut runner.runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
    states.register_compressed_legacy_state(CompressedLegacyState::Protect);
    let payloads = [
        StatePayload::Poison {
            caster: Some(1),
            target: Some(0),
            atp_bits: 123.5f64.to_bits(),
            count: 3,
        },
        StatePayload::Charm {
            group_id: 1,
            effective_team_idx: Some(1),
            source_team_idx: Some(0),
            target: Some(1),
            step: 2,
        },
        StatePayload::CovidInfection {
            entries: vec![CovidInfectionEntry {
                boss: EntityIdx(1),
                mutation: 7,
                days: 2,
            }]
            .into(),
            mutation_set: vec![7].into(),
            recovered: false,
        },
        StatePayload::SaitamaBoss {
            turns: 8,
            damages: 99,
            hitters: vec![EntityIdx(1)].into(),
            minions: vec![EntityIdx(0)].into(),
        },
        StatePayload::LazyInfection { boss: EntityIdx(1) },
        StatePayload::Iron { protect: 15, step: 2 },
    ];
    for (index, payload) in payloads.iter().enumerate() {
        let mut entry = StateEntry::legacy(100 + index as u32);
        entry.payload = payload.clone();
        states.add_entry(entry);
    }
    let exported = runner.model_state().unwrap();
    let entity = &exported.entities[0];
    assert_ne!(entity.compressed_state_flags & (1 << 1), 0);
    assert_eq!(entity.states.len(), payloads.len());
    assert_eq!(entity.state_registration_cursor, 7);
    for (index, state) in entity.states.iter().enumerate() {
        assert_eq!(state.runtime_registration_order, index as u64 + 1);
        assert_eq!(state.payload, ModelPayload::from(&payloads[index]));
    }
    assert_eq!(entity.states[0].payload.poison.as_ref().unwrap().atp_bits, 123.5f64.to_bits());
}

#[test]
fn entity_list_order_does_not_replace_action_order_or_team_identity() {
    let mut state = runner().model_state().unwrap();
    let order = state.world.round_order.clone();
    state.entities.reverse();
    state.validate().unwrap();
    assert_eq!(state.world.round_order, order);
    for entity in &state.entities {
        assert!(state.input_teams[entity.input_team_index].contains(&entity.runtime.root_owner));
    }
    state.entities[0].runtime.root_owner = EntityIdx(u32::MAX);
    assert!(state.validate().is_err());
}

#[test]
fn builtin_bosses_export_name_derived_mechanics() {
    for name in crate::namerena::BOSS_NAMES {
        let mut runner = RuntimeRunner::new_from_namerena_raw(format!("{name}@!\n\nleft\nright")).unwrap();
        for _ in 0..20 {
            let state = runner.model_state().unwrap();
            let boss = &state.entities[0].template.identity;
            assert_eq!(
                boss.boss_kind,
                crate::namerena::BOSS_NAMES.iter().position(|candidate| *candidate == name)
            );
            assert_eq!(
                boss.immunity.iter().find(|entry| entry.status == "charm").unwrap().threshold,
                crate::namerena::boss_immune_threshold(name, "charm")
            );
            if runner.have_winner() {
                break;
            }
            runner.main_round();
        }
    }
}

#[test]
fn lazy_blueprint_projection_does_not_fill_cache_and_matches_materialized_state() {
    let mut runner = runner();
    let registry = &runner.runtime.registry;
    let blueprint = registry
        .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT)
        .unwrap();
    let lazy = registry
        .entity_slot_id_by_export_name(DEFAULT_CORE_LAZY_BLUEPRINT_RQ_ENTITY_EXPORT)
        .unwrap();
    let entity = runner.runtime.entities.get_mut(EntityIdx(0)).unwrap();
    entity.slots.remove(blueprint);
    entity
        .slots
        .set(lazy, SlotValue::U64(crate::namerena::eval_name::DEFAULT_EVAL_RQ.to_bits()))
        .unwrap();
    let rng = runner.runtime.rng.clone();
    let before = runner.model_state().unwrap();
    assert!(runner.runtime.entities.get(EntityIdx(0)).unwrap().slots.get(blueprint).is_none());
    assert!(
        runner
            .runtime
            .ensure_plain_minion_blueprint(EntityIdx(0), crate::namerena::MinionKind::Summon)
    );
    assert_eq!(before, runner.model_state().unwrap());
    assert_eq!(rng.main_val, runner.runtime.rng.main_val);
    assert_eq!((rng.i, rng.j), (runner.runtime.rng.i, runner.runtime.rng.j));
}

#[test]
fn unsupported_registry_and_invalid_blueprint_are_errors() {
    let mut runner = runner();
    let blueprint = runner
        .runtime
        .registry
        .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT)
        .unwrap();
    runner
        .runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .slots
        .set(blueprint, SlotValue::Text("unsupported".into()))
        .unwrap();
    assert!(runner.model_state().unwrap_err().to_string().contains("类型无效"));
    runner.runtime.registry = ExtensionRegistry::default();
    assert!(runner.model_state().unwrap_err().to_string().contains("默认规则注册表"));
}

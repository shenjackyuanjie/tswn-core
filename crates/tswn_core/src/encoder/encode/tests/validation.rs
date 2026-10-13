use super::*;

#[test]
fn entity_capacity_boundary_is_enforced() {
    let state = battle_state(2);
    let encoder = encoder();
    let base = state.entities.len();
    assert!(
        encoder.encode(&replicate_entities(&state, 64 - base)).is_ok(),
        "E=64 必须可编码"
    );
    assert!(
        matches!(
            encoder.encode(&replicate_entities(&state, 65 - base)).unwrap_err(),
            EncodeError::CapacityExceeded { ref path, actual: 65, limit: 64 } if path == "entities"
        ),
        "E=65 必须报 CapacityExceeded"
    );
}

#[test]
fn input_team_capacity_boundary_is_enforced() {
    let state = battle_state(2);
    let encoder = encoder();
    let mut at_limit = state.clone();
    let member = at_limit.entities[0].runtime.root_owner;
    while at_limit.input_teams.len() < 32 {
        at_limit.input_teams.push(vec![member]);
    }
    assert!(encoder.encode(&at_limit).is_ok(), "T=32 必须可编码");
    at_limit.input_teams.push(vec![member]);
    assert!(matches!(
        encoder.encode(&at_limit).unwrap_err(),
        EncodeError::CapacityExceeded { ref path, actual: 33, limit: 32 } if path == "input_teams"
    ));
}

#[test]
fn runtime_team_capacity_boundary_is_enforced() {
    let state = battle_state(2);
    let encoder = encoder();
    let base = state.entities.len();
    // 30 个实体各占一个 runtime team 原值，加上 world 两行 = 32，恰好达标。
    let mut at_limit = replicate_entities(&state, 30 - base);
    for (index, entity) in at_limit.entities.iter_mut().enumerate() {
        entity.runtime.team = 100 + index;
    }
    assert_eq!(CapacityMeasure::measure(&at_limit).r, 32);
    assert!(encoder.encode(&at_limit).is_ok(), "R=32 必须可编码");
    let mut over = replicate_entities(&state, 31 - base);
    for (index, entity) in over.entities.iter_mut().enumerate() {
        entity.runtime.team = 100 + index;
    }
    assert_eq!(CapacityMeasure::measure(&over).r, 33);
    assert!(matches!(
        encoder.encode(&over).unwrap_err(),
        EncodeError::CapacityExceeded { ref path, limit: 32, .. } if path == "runtime_team"
    ));
}

#[test]
fn runtime_team_measure_matches_written_union_with_blueprints_and_empty_rows() {
    let mut state = battle_state(0);
    state.world.team_roster.resize(3, Vec::new());
    state.world.team_alive.resize(4, Vec::new());
    for entity in &mut state.entities {
        entity.runtime.team = 2;
        entity.template.team = 6;
        entity.slots.clear();
    }
    state.template_slots.clear();
    state.battle_slots.clear();
    let mut blueprint = state.entities[0].template.clone();
    blueprint.team = 1000;
    state.entities[0].slots.push(crate::runtime::model_state::ModelSlot {
        slot_id: 0,
        bool_value: None,
        i64_value: None,
        u64_value: None,
        template: Some(blueprint.clone()),
    });
    blueprint.team = 2000;
    state.template_slots.push(crate::runtime::model_state::ModelSlot {
        slot_id: 0,
        bool_value: None,
        i64_value: None,
        u64_value: None,
        template: Some(blueprint),
    });
    // 集合为 {0,1,2,3,6,1000,2000}；计量的是关系行数，不是最大原编号加一。
    let measure = CapacityMeasure::measure(&state);
    assert_eq!(measure.r, 7);
    let batch = encoder().encode(&state).unwrap();
    let mask = batch.u8_all("runtime_team_mask").unwrap();
    assert_eq!(&mask[..7], &[1; 7]);
    assert!(mask[7..].iter().all(|value| *value == 0));
    for row in 0..measure.e {
        assert_eq!(batch.i32_all("entity_team").unwrap()[2 * row + 1], 2);
        assert_eq!(batch.i32_all("template_team").unwrap()[row], 4);
    }
    assert_eq!(batch.i32_all("template_team").unwrap()[measure.e], 5);
    assert_eq!(batch.i32_all("template_team").unwrap()[measure.e + 1], 6);
    assert_eq!(measure.h, measure.e + 2);
    assert_eq!(
        batch.u8_all("template_mask").unwrap().iter().filter(|value| **value == 1).count(),
        measure.h
    );
}

#[test]
fn slot_capacity_boundary_is_enforced() {
    let state = battle_state(2);
    let encoder = encoder();
    let base = CapacityMeasure::measure(&state).q;
    let mut at_limit = state.clone();
    for index in 0..(512 - base) {
        at_limit.entities[0].slots.push(counter_slot(index as u64));
    }
    assert_eq!(CapacityMeasure::measure(&at_limit).q, 512);
    assert!(encoder.encode(&at_limit).is_ok(), "Q=512 必须可编码");
    let mut over = at_limit.clone();
    over.entities[0].slots.push(counter_slot(1));
    assert!(matches!(
        encoder.encode(&over).unwrap_err(),
        EncodeError::CapacityExceeded { ref path, actual: 513, limit: 512 } if path == "slots"
    ));
}

// ------------------------------------------------------------ 门禁与拒绝路径

#[test]
fn already_decided_state_is_rejected() {
    let mut state = battle_state(2);
    state.world.winner_team = Some(0);
    assert!(matches!(
        encoder().encode(&state).unwrap_err(),
        EncodeError::AlreadyDecided { .. }
    ));
}

#[test]
fn schema_mismatch_is_rejected() {
    let mut state = battle_state(2);
    state.schema_version = MODEL_STATE_SCHEMA_VERSION + 1;
    assert!(matches!(
        encoder().encode(&state).unwrap_err(),
        EncodeError::SchemaMismatch { .. }
    ));
}

#[test]
fn entity_ids_must_fit_entity_slot_count() {
    let mut state = battle_state(0);
    let max_id = state.entities.iter().map(|entity| entity.id.0 as usize).max().unwrap();
    state.entity_slot_count = max_id + 1;
    assert!(encoder().encode(&state).is_ok());
    for invalid_count in [max_id, 0] {
        state.entity_slot_count = invalid_count;
        assert!(matches!(
            encoder().encode(&state).unwrap_err(),
            EncodeError::InvalidState { ref path } if path.ends_with(".id")
        ));
    }
}

#[test]
fn reserved_flag_bits_are_rejected_and_named_bits_are_written() {
    let mut state = battle_state(2);
    state.entities[0].compressed_state_flags = 0b0001_0101;
    let batch = encoder().encode(&state).unwrap();
    let flags = batch.u8_all("entity_flags").unwrap();
    assert_eq!(&flags[..8], &[1, 0, 1, 0, 1, 0, 0, 0]);
    state.entities[0].compressed_state_flags = 0b0010_0000;
    assert!(matches!(
        encoder().encode(&state).unwrap_err(),
        EncodeError::ReservedFlagBitSet { .. }
    ));
}

#[test]
fn non_finite_value_is_rejected() {
    let encoder = encoder();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut state = battle_state(2);
        state.entities[0].runtime.at_boost_bits = value.to_bits();
        assert!(matches!(
            encoder.encode(&state).unwrap_err(),
            EncodeError::NonFiniteValue { .. }
        ));
        let mut state = battle_state(2);
        state.entities[0].template.at_boost_bits = value.to_bits();
        assert!(matches!(
            encoder.encode(&state).unwrap_err(),
            EncodeError::NonFiniteValue { .. }
        ));
    }
}

#[test]
fn unknown_category_value_is_rejected() {
    let mut state = battle_state(2);
    state.entities[0].template.kind = crate::runtime::extension::PlayerKindId(9999);
    assert!(matches!(
        encoder().encode(&state).unwrap_err(),
        EncodeError::UnknownCategory { .. }
    ));
}

#[test]
#[cfg(target_pointer_width = "64")]
fn oversized_boss_kind_does_not_alias_a_valid_category() {
    let mut state = battle_state(0);
    state.entities[0].template.identity.boss_kind = Some((1u64 << 32) as usize);
    assert!(matches!(
        encoder().encode(&state).unwrap_err(),
        EncodeError::UnknownCategory { ref path, ref raw }
            if path == "templates[0].identity.boss_kind" && raw == "4294967296"
    ));
}

#[test]
fn payload_kind_and_branch_must_match() {
    let mut state = battle_state(2);
    state.entities[0].states.push(crate::runtime::model_state::ModelStateEntry {
        legacy_order_key: 1,
        extension_state_id: None,
        hook_mask: 0,
        priority: 0,
        registration_order: 1,
        runtime_registration_order: 1,
        payload: ModelPayload {
            kind: "none".to_owned(),
            ..ModelPayload::default()
        },
    });
    assert!(encoder().encode(&state).is_ok(), "kind=none 且无分支必须可编码");
    let mut boss = state.clone();
    boss.entities[0].states[0].payload.kind = "covid_boss".to_owned();
    boss.entities[0].states[0].payload.covid_boss = Some(crate::runtime::model_state::CovidBossPayload { mutation: 1 });
    assert!(matches!(
        encoder().encode(&boss).unwrap_err(),
        EncodeError::UnsupportedPayloadKind { .. }
    ));
    let mut mismatched = state.clone();
    mismatched.entities[0].states[0].payload.kind = "poison".to_owned();
    assert!(matches!(
        encoder().encode(&mismatched).unwrap_err(),
        EncodeError::InvalidState { .. }
    ));
    let mut extra = state.clone();
    extra.entities[0].states[0].payload.shield_value = Some(3);
    assert!(matches!(
        encoder().encode(&extra).unwrap_err(),
        EncodeError::InvalidState { .. }
    ));
}

#[test]
fn unknown_state_categories_are_rejected() {
    let mut legacy = battle_state(2);
    legacy.entities[0].states.push(crate::runtime::model_state::ModelStateEntry {
        legacy_order_key: 2,
        extension_state_id: None,
        hook_mask: 0,
        priority: 0,
        registration_order: 1,
        runtime_registration_order: 1,
        payload: ModelPayload {
            kind: "none".to_owned(),
            ..ModelPayload::default()
        },
    });
    assert!(matches!(
        encoder().encode(&legacy).unwrap_err(),
        EncodeError::UnknownCategory { ref path, .. } if path.ends_with(".legacy_order_key")
    ));

    let mut extension = battle_state(2);
    extension.entities[0].states.push(crate::runtime::model_state::ModelStateEntry {
        legacy_order_key: 1,
        extension_state_id: Some(9999),
        hook_mask: 0,
        priority: 0,
        registration_order: 1,
        runtime_registration_order: 1,
        payload: ModelPayload {
            kind: "none".to_owned(),
            ..ModelPayload::default()
        },
    });
    assert!(matches!(
        encoder().encode(&extension).unwrap_err(),
        EncodeError::UnknownCategory { ref path, .. } if path.ends_with(".extension_state_id")
    ));
}

#[test]
fn slot_whitelist_is_enforced() {
    let mut state = battle_state(2);
    state.entities[0].slots.push(crate::runtime::model_state::ModelSlot {
        slot_id: 9,
        bool_value: None,
        i64_value: None,
        u64_value: Some(1),
        template: None,
    });
    assert!(matches!(
        encoder().encode(&state).unwrap_err(),
        EncodeError::UnknownSlotSemantics { .. }
    ));
    let mut dangling = battle_state(2);
    dangling.entities[0].slots.push(crate::runtime::model_state::ModelSlot {
        slot_id: 4,
        bool_value: None,
        i64_value: None,
        u64_value: Some(4242),
        template: None,
    });
    assert!(matches!(
        encoder().encode(&dangling).unwrap_err(),
        EncodeError::InvalidReference { .. }
    ));
}

#[test]
fn charm_group_reference_is_validated() {
    let mut state = battle_state(2);
    state.entities[0].states.push(crate::runtime::model_state::ModelStateEntry {
        legacy_order_key: 1,
        extension_state_id: None,
        hook_mask: 0,
        priority: 0,
        registration_order: 1,
        runtime_registration_order: 1,
        payload: ModelPayload {
            kind: "charm".to_owned(),
            charm: Some(crate::runtime::model_state::CharmPayload {
                group_id: usize::MAX,
                effective_team_idx: None,
                source_team_idx: None,
                target: None,
                step: 1,
            }),
            ..ModelPayload::default()
        },
    });
    assert!(matches!(
        encoder().encode(&state).unwrap_err(),
        EncodeError::InvalidReference { .. }
    ));
}

// ------------------------------------------------------------ presence / mask 语义

#[test]
fn missing_calibration_is_reported_at_construction() {
    let mut incomplete = manifest();
    incomplete.normalization.remove("entity.runtime.hide.level");
    assert!(matches!(
        FeatureEncoder::new(incomplete).unwrap_err(),
        EncodeError::MissingCalibration { ref path } if path == "entity.runtime.hide.level"
    ));
}

#[test]
fn vocabulary_must_match_default_registry() {
    let mut tampered = manifest();
    tampered.vocabularies.get_mut("runtime.kind").unwrap().entries.insert(4321, 99);
    assert!(matches!(
        FeatureEncoder::new(tampered).unwrap_err(),
        EncodeError::ManifestMismatch { .. }
    ));
}

#[test]
fn wrong_transform_kind_is_rejected() {
    let mut tampered = manifest();
    tampered.normalization.get_mut("global.entity_slot_count").unwrap().transform = TransformKind::Fitted;
    assert!(matches!(
        FeatureEncoder::new(tampered).unwrap_err(),
        EncodeError::ManifestMismatch { .. }
    ));
}

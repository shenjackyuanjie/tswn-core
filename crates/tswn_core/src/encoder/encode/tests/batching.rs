use super::*;

#[test]
fn encoding_is_deterministic_within_one_build() {
    let state = battle_state(8);
    let encoder = encoder();
    let first = encoder.encode(&state).unwrap();
    let second = encoder.encode(&state).unwrap();
    for spec in TENSOR_SPECS {
        assert_eq!(
            tensor_bytes(&first, spec.name),
            tensor_bytes(&second, spec.name),
            "{}",
            spec.name
        );
    }
}

// ------------------------------------------------------------ 容量边界

#[test]
fn batch_slot_matches_standalone_encoding() {
    let state = battle_state(6);
    let encoder = encoder();
    let standalone = encoder.encode(&state).unwrap();
    let mut batch = EncodedBatch::new(encoder.profile(), 3);
    encoder.encode_into(&state, 2, &mut batch).unwrap();
    for spec in TENSOR_SPECS {
        let shape = crate::encoder::batch::tensor_shape(&encoder.profile().dims(), spec.name).unwrap();
        let per_sample: usize = shape.iter().product();
        let element = spec.dtype.size();
        assert_eq!(
            tensor_bytes(&standalone, spec.name),
            tensor_bytes(&batch, spec.name)[2 * per_sample * element..3 * per_sample * element],
            "{} 的批槽位必须与 B=1 逐字节一致",
            spec.name
        );
    }
}

#[test]
fn failed_encode_clears_only_target_slot_and_allows_reuse() {
    let valid = battle_state(0);
    let encoder = encoder();
    let standalone = encoder.encode(&valid).unwrap();
    let padding = EncodedBatch::new(encoder.profile(), 1);
    for case in 0..5 {
        let mut invalid = valid.clone();
        match case {
            0 => invalid.schema_version += 1,
            1 => invalid.entities[0].runtime.at_boost_bits = f64::NAN.to_bits(),
            2 => invalid.entities[0].template.kind = crate::runtime::extension::PlayerKindId(9999),
            3 => {
                let entry = crate::runtime::model_state::ModelImmunity {
                    status: "fire".to_owned(),
                    threshold: 0,
                };
                invalid.entities[0].template.identity.immunity = vec![entry.clone(), entry];
            }
            _ => invalid.entities[0].slots = vec![counter_slot(1); encoder.profile().q_max + 1],
        }
        let mut batch = EncodedBatch::new(encoder.profile(), 3);
        // 所有 dtype、所有张量都写入哨兵，确保清理错误不会被本来就是 padding 的邻居掩盖。
        for spec in TENSOR_SPECS {
            for slot in 0..3 {
                match spec.dtype {
                    Dtype::F32 => batch.f32_row_mut(spec.name, slot).unwrap().fill((slot + 1) as f32),
                    Dtype::I32 => batch.i32_row_mut(spec.name, slot).unwrap().fill((slot + 1) as i32),
                    Dtype::U8 => batch.u8_row_mut(spec.name, slot).unwrap().fill((slot + 1) as u8),
                    Dtype::U32 => batch.u32_row_mut(spec.name, slot).unwrap().fill((slot + 1) as u32),
                }
            }
        }
        encoder.encode_into(&valid, 1, &mut batch).unwrap();
        let before: Vec<_> = TENSOR_SPECS.iter().map(|spec| tensor_bytes(&batch, spec.name)).collect();
        let error = encoder.encode_into(&invalid, 1, &mut batch).unwrap_err();
        match case {
            0 => assert!(matches!(error, EncodeError::SchemaMismatch { .. })),
            1 => assert!(matches!(error, EncodeError::NonFiniteValue { .. })),
            2 => assert!(matches!(error, EncodeError::UnknownCategory { .. })),
            3 => assert!(matches!(error, EncodeError::DuplicateCategory { .. })),
            _ => assert!(matches!(error, EncodeError::CapacityExceeded { ref path, .. } if path == "slots")),
        }
        for (spec, previous) in TENSOR_SPECS.iter().zip(&before) {
            let empty = tensor_bytes(&padding, spec.name);
            let stride = empty.len();
            let after = tensor_bytes(&batch, spec.name);
            assert_eq!(&after[..stride], &previous[..stride], "{} 左邻居被修改", spec.name);
            assert_eq!(&after[stride..2 * stride], empty.as_slice(), "{} 失败后不是 padding", spec.name);
            assert_eq!(&after[2 * stride..], &previous[2 * stride..], "{} 右邻居被修改", spec.name);
        }
        encoder.encode_into(&valid, 1, &mut batch).unwrap();
        for (spec, previous) in TENSOR_SPECS.iter().zip(&before) {
            let expected = tensor_bytes(&standalone, spec.name);
            let stride = expected.len();
            let after = tensor_bytes(&batch, spec.name);
            assert_eq!(
                &after[stride..2 * stride],
                expected.as_slice(),
                "{} 失败后复用不一致",
                spec.name
            );
            assert_eq!(&after[..stride], &previous[..stride]);
            assert_eq!(&after[2 * stride..], &previous[2 * stride..]);
        }
    }
}

#[test]
fn out_of_range_encode_does_not_modify_batch() {
    let state = battle_state(0);
    let encoder = encoder();
    let mut batch = encoder.encode(&state).unwrap();
    let before: Vec<_> = TENSOR_SPECS.iter().map(|spec| tensor_bytes(&batch, spec.name)).collect();
    assert!(matches!(
        encoder.encode_into(&state, 1, &mut batch).unwrap_err(),
        EncodeError::BatchSlotOutOfRange { batch: 1, limit: 1 }
    ));
    for (spec, expected) in TENSOR_SPECS.iter().zip(before) {
        assert_eq!(tensor_bytes(&batch, spec.name), expected, "{} 被越界调用修改", spec.name);
    }
}

#[test]
fn reused_slot_does_not_keep_stale_values() {
    let mut rich = battle_state(8);
    rich.entities[0].runtime.protect_to = Some(rich.entities[1].id);
    rich.entities[0].runtime.hide = Some(crate::runtime::entity::HideRuntime {
        level: 2,
        attract_bits: 1.5f64.to_bits(),
        agility: 1,
        defense: 2,
        resistance: 3,
    });
    let encoder = encoder();
    let mut batch = EncodedBatch::new(encoder.profile(), 1);
    encoder.encode_into(&rich, 0, &mut batch).unwrap();
    let poor = battle_state(2);
    assert!(poor.entities.len() < rich.entities.len(), "测试需要实体数更少的样本");
    encoder.encode_into(&poor, 0, &mut batch).unwrap();
    assert!(
        batch.u8_all("entity_mask").unwrap()[poor.entities.len()..]
            .iter()
            .all(|value| *value == 0),
        "padding 行必须被清掉"
    );
    assert!(
        batch.i32_all("entity_ref").unwrap()[poor.entities.len() * 5..]
            .iter()
            .all(|value| *value == -1),
        "引用 padding 必须为 -1"
    );
    assert_eq!(
        batch.u8_all("entity_bool").unwrap()[7],
        0,
        "上一个样本的 hide presence 不得残留"
    );
}

#[test]
fn batch_and_encoder_profiles_must_match() {
    let state = battle_state(2);
    let encoder = encoder();
    let mut smaller = BASELINE_64;
    smaller.e_max = 32;
    smaller.h_max = 256;
    let mut batch = EncodedBatch::new(&smaller, 1);
    batch.u8_row_mut("entity_mask", 0).unwrap()[0] = 1;
    assert!(matches!(
        encoder.encode_into(&state, 0, &mut batch).unwrap_err(),
        EncodeError::ManifestMismatch { .. }
    ));
    assert_eq!(batch.u8_all("entity_mask").unwrap()[0], 1, "profile 错配不能修改缓冲");
}

// ------------------------------------------------------------ manifest 门禁

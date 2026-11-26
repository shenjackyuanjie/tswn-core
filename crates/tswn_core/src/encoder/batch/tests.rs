use super::*;

#[test]
fn registry_names_are_unique_and_have_shapes() {
    let mut seen = std::collections::BTreeSet::new();
    for spec in TENSOR_SPECS {
        assert!(seen.insert(spec.name), "张量名重复：{}", spec.name);
        assert!(
            tensor_shape(&BASELINE_64.dims(), spec.name).is_some(),
            "{} 缺少 shape 推导",
            spec.name
        );
    }
}

#[test]
fn allocation_matches_shape_product_and_padding_values() {
    let batch = EncodedBatch::baseline(2);
    for spec in TENSOR_SPECS {
        let shape = tensor_shape(&BASELINE_64.dims(), spec.name).unwrap();
        let product: usize = shape.iter().product();
        assert_eq!(
            batch.tensor_len(spec.name).unwrap(),
            product * 2,
            "{} 的元素数必须等于 shape 之积 × batch",
            spec.name
        );
        match spec.fill {
            Fill::Zero => match spec.dtype {
                Dtype::F32 => assert!(batch.f32_all(spec.name).unwrap().iter().all(|value| *value == 0.0)),
                Dtype::I32 => assert!(batch.i32_all(spec.name).unwrap().iter().all(|value| *value == 0)),
                Dtype::U8 => assert!(batch.u8_all(spec.name).unwrap().iter().all(|value| *value == 0)),
                Dtype::U32 => assert!(batch.u32_all(spec.name).unwrap().iter().all(|value| *value == 0)),
            },
            Fill::RefMinusOne => {
                assert!(
                    batch.i32_all(spec.name).unwrap().iter().all(|value| *value == -1),
                    "{} 的 padding 必须是 -1",
                    spec.name
                );
            }
            Fill::Index { .. } => {
                for (index, value) in batch.i32_all(spec.name).unwrap().iter().enumerate() {
                    assert_eq!(*value, spec.fill.i32_at(index), "{} 的混合列 padding", spec.name);
                }
            }
        }
    }
}

#[test]
fn batch_slot_offset_is_per_tensor() {
    let mut batch = EncodedBatch::baseline(3);
    batch.u8_row_mut("entity_mask", 1).unwrap()[0] = 7;
    batch.i32_row_mut("entity_ref", 2).unwrap()[3] = 9;
    let mask = batch.u8_all("entity_mask").unwrap();
    assert_eq!(mask[0], 0);
    assert_eq!(mask[BASELINE_64.e_max], 7, "槽位 1 的偏移必须是 e_max");
    assert_eq!(mask[2 * BASELINE_64.e_max], 0);
    let reference = batch.i32_all("entity_ref").unwrap();
    assert_eq!(reference[3], -1);
    assert_eq!(reference[2 * 5 * BASELINE_64.e_max + 3], 9);
}

#[test]
fn clear_slot_restores_padding() {
    let mut batch = EncodedBatch::baseline(1);
    {
        let mask = batch.u8_row_mut("entity_mask", 0).unwrap();
        mask[0] = 1;
        mask[5] = 1;
    }
    {
        let reference = batch.i32_row_mut("entity_ref", 0).unwrap();
        reference[0] = 3;
    }
    batch.clear_slot(0).unwrap();
    assert!(batch.u8_all("entity_mask").unwrap().iter().all(|value| *value == 0));
    assert!(batch.i32_all("entity_ref").unwrap().iter().all(|value| *value == -1));
}

#[test]
fn clear_middle_slot_does_not_touch_other_batch_slots() {
    let mut batch = EncodedBatch::baseline(3);
    batch.f32_row_mut("global_num", 0).unwrap()[0] = 1.0;
    batch.f32_row_mut("global_num", 1).unwrap()[0] = 2.0;
    batch.f32_row_mut("global_num", 2).unwrap()[0] = 3.0;
    batch.i32_row_mut("entity_ref", 0).unwrap()[0] = 10;
    batch.i32_row_mut("entity_ref", 1).unwrap()[0] = 20;
    batch.i32_row_mut("entity_ref", 2).unwrap()[0] = 30;

    batch.clear_slot(1).unwrap();

    assert_eq!(batch.f32_row_mut("global_num", 0).unwrap()[0], 1.0);
    assert_eq!(batch.f32_row_mut("global_num", 1).unwrap()[0], 0.0);
    assert_eq!(batch.f32_row_mut("global_num", 2).unwrap()[0], 3.0);
    assert_eq!(batch.i32_row_mut("entity_ref", 0).unwrap()[0], 10);
    assert_eq!(batch.i32_row_mut("entity_ref", 1).unwrap()[0], -1);
    assert_eq!(batch.i32_row_mut("entity_ref", 2).unwrap()[0], 30);
}

#[test]
fn mixed_index_padding_is_column_typed_and_slot_local() {
    for (name, expected) in [
        ("list_index", &[0, -1, 0, 0, -1][..]),
        ("slot_index", &[0, -1, 0, 0][..]),
        ("extra_index", &[0, -1, 0, 0][..]),
    ] {
        let mut batch = EncodedBatch::baseline(3);
        assert!(batch.i32_all(name).unwrap().chunks_exact(expected.len()).all(|row| row == expected));
        for slot in 0..3 {
            batch.i32_row_mut(name, slot).unwrap().fill(20 + slot as i32);
        }
        batch.clear_slot(1).unwrap();
        assert!(batch.i32_row_mut(name, 0).unwrap().iter().all(|value| *value == 20));
        assert!(
            batch
                .i32_row_mut(name, 1)
                .unwrap()
                .chunks_exact(expected.len())
                .all(|row| row == expected)
        );
        assert!(batch.i32_row_mut(name, 2).unwrap().iter().all(|value| *value == 22));
    }
}

#[test]
fn slot_scalar_tensors_keep_the_singleton_feature_axis() {
    for name in ["slot_value", "slot_value_present"] {
        assert_eq!(tensor_shape(&BASELINE_64.dims(), name), Some(vec![BASELINE_64.q_max, 1]));
    }
}

#[test]
fn byte_len_matches_shape_times_dtype_size() {
    let batch = EncodedBatch::baseline(4);
    for spec in TENSOR_SPECS {
        let shape = tensor_shape(&BASELINE_64.dims(), spec.name).unwrap();
        let product: usize = shape.iter().product();
        let expected = product * 4 * spec.dtype.size();
        assert_eq!(batch.tensor_byte_len(spec.name).unwrap(), expected, "{}", spec.name);
    }
}

#[test]
fn row_accessors_reject_out_of_range_batch_and_unknown_tensor() {
    let mut batch = EncodedBatch::baseline(2);
    assert!(matches!(
        batch.u8_row_mut("entity_mask", 2).unwrap_err(),
        EncodeError::BatchSlotOutOfRange { batch: 2, limit: 2 }
    ));
    assert!(matches!(
        batch.u8_row_mut("not_a_tensor", 0).unwrap_err(),
        EncodeError::UnknownTensor { .. }
    ));
}

#[test]
fn wrong_dtype_access_is_rejected() {
    let batch = EncodedBatch::baseline(1);
    // entity_mask 是 u8；用 f32 视图访问必须报未知张量而不是 reinterpret。
    assert!(matches!(
        batch.f32_all("entity_mask").unwrap_err(),
        EncodeError::UnknownTensor { .. }
    ));
}

#[test]
fn tensor_bytes_are_little_endian_and_match_declared_length() {
    let batch = EncodedBatch::baseline(1);
    for spec in TENSOR_SPECS {
        let bytes = batch.tensor_bytes(spec.name).unwrap();
        assert_eq!(bytes.len(), batch.tensor_byte_len(spec.name).unwrap(), "{}", spec.name);
    }
    let bytes = batch.tensor_bytes("list_index").unwrap();
    assert_eq!(
        &bytes[..20],
        &[
            0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff
        ]
    );
}

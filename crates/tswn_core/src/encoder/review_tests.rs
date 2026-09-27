// 联合编码回归：由 encode.rs 的 tests 模块 include，复用其校准与字节比较助手。
// 期望值按规格的 owner 域和源行独立构造，不能只与同一个编码器的另一次输出比较。

fn review_state() -> BattleModelState {
    use crate::runtime::model_state::{ModelSkill, ModelSkillBoost, ModelSkills};
    let mut state = battle_state(0);
    assert_eq!(state.entities.len(), 2);
    state.template_slots.clear();
    state.battle_slots.clear();
    for entity in &mut state.entities {
        entity.states.clear();
        entity.slots.clear();
        entity.state_registration_cursor = 0;
        entity.runtime.hide = None;
        entity.runtime.assassinate = None;
        entity.runtime.protect_from.clear();
        entity.runtime.protect_to = None;
        entity.runtime.counter.last_target = None;
        entity.template.clone_build = None;
        entity.template.skills = ModelSkills {
            lanes: vec![
                ModelSkill { skill_id: 6, level: 7, build_level: 7, boost: None, boosted: false, fixed_lane_key: 88 },
                ModelSkill {
                    skill_id: 6,
                    level: 0,
                    build_level: 0,
                    boost: Some(ModelSkillBoost { kind: "normal".to_owned(), base: 0, extra: 0 }),
                    boosted: false,
                    fixed_lane_key: 11,
                },
            ],
            merge_lane_order: vec![1, 0],
            active_order: vec![1, 0, 1],
            pre_action_order: Vec::new(),
            post_damage_order: vec![0],
            post_action_after_states: Vec::new(),
        };
    }
    state
}

fn review_entry(payload: ModelPayload, runtime_order: u64) -> crate::runtime::model_state::ModelStateEntry {
    crate::runtime::model_state::ModelStateEntry {
        legacy_order_key: 1,
        extension_state_id: None,
        hook_mask: 0,
        priority: 0,
        registration_order: 1,
        runtime_registration_order: runtime_order,
        payload,
    }
}

fn review_blueprint(slot_id: u32, template: ModelTemplate) -> crate::runtime::model_state::ModelSlot {
    crate::runtime::model_state::ModelSlot {
        slot_id,
        bool_value: None,
        i64_value: None,
        u64_value: None,
        template: Some(template),
    }
}

fn review_extra_row(batch: &EncodedBatch, expected: [i32; 4]) -> usize {
    let indices = batch.i32_all("extra_index").unwrap();
    let mask = batch.u8_all("extra_mask").unwrap();
    let rows: Vec<_> = indices
        .chunks_exact(4)
        .enumerate()
        .filter(|(row, key)| mask[*row] == 1 && *key == expected.as_slice())
        .map(|(row, _)| row)
        .collect();
    assert_eq!(rows.len(), 1, "语义键必须唯一且存在：{expected:?}，实际行 {rows:?}");
    rows[0]
}

fn review_raw(batch: &EncodedBatch, key: [i32; 4]) -> u64 {
    let row = review_extra_row(batch, key);
    let bits = batch.u32_all("extra_bits").unwrap();
    u64::from(bits[row * 2]) | (u64::from(bits[row * 2 + 1]) << 32)
}

fn review_plan() -> crate::runtime::entity::CloneBuildData {
    use crate::runtime::entity::{CloneBuildData, CloneStatAdjustments, ScoreCloneSkillBoostPlan};
    CloneBuildData {
        attrs: [0; 8],
        weapon_attr_bonus: [0; 8],
        name_factor_bits: 1.0f64.to_bits(),
        child_name_factor_bits: 1.0f64.to_bits(),
        adjustments: CloneStatAdjustments {
            max_hp: 0,
            attack: 0,
            magic: 0,
            wisdom: 0,
            speed: 0,
            defense: 0,
            resistance: 0,
            agility: 0,
            at_boost_delta_bits: 0.0f64.to_bits(),
            attr_sum: 0,
            atk_sum: 0,
            attract_delta_bits: 0.0f64.to_bits(),
        },
        score_skill_boost_plan: Some(ScoreCloneSkillBoostPlan {
            initially_boosted_mask: 0,
            slot_boosts: [Some((0, 7)), Some((9, 0))],
        }),
    }
}

#[test]
fn slot_raw_and_entity_ref_owners_join_the_actual_slot_rows() {
    let mut state = review_state();
    state.entities[0].slots.push(counter_slot(u64::MAX));
    let mut reference = counter_slot(u64::from(state.entities[1].id.0));
    reference.slot_id = 4;
    state.entities[0].slots.push(reference);
    state.entities[1].slots.push(counter_slot(0));
    let template = state.entities[0].template.clone();
    state.template_slots.push(review_blueprint(0, template.clone()));
    state.template_slots.push(review_blueprint(1, template));
    let batch = encoder().encode(&state).unwrap();
    let expected = [[1, 0, 6, 3], [1, 0, 5, 3], [1, 1, 6, 3], [2, 0, 1, 4], [2, 0, 2, 4]];
    for (row, index) in expected.iter().enumerate() {
        assert_eq!(&batch.i32_all("slot_index").unwrap()[4 * row..4 * row + 4], index);
    }
    for (row, raw_id) in [5u64, 4, 5, 0, 1].into_iter().enumerate() {
        assert_eq!(review_raw(&batch, [5, row as i32, 4101, 0]), raw_id);
    }
    assert_eq!(review_raw(&batch, [5, 0, 4103, 0]), u64::MAX);
    assert_eq!(review_raw(&batch, [5, 2, 4103, 0]), 0);
    let reference = review_extra_row(&batch, [5, 1, 8, 0]);
    assert_eq!(batch.i32_all("extra_ref").unwrap()[reference], 1);
    assert_eq!(batch.u8_all("slot_value_present").unwrap()[1], 0);
    assert_eq!(&batch.u8_all("slot_field_present").unwrap()[4..8], &[0, 0, 1, 0]);
    assert_eq!(batch.i32_all("slot_template").unwrap()[3], 2);
    assert_eq!(batch.i32_all("slot_template").unwrap()[4], 3, "相等模板的两个源实例仍是两行");
    let mut seen = BTreeSet::new();
    for (row, key) in batch.i32_all("extra_index").unwrap().chunks_exact(4).enumerate() {
        if batch.u8_all("extra_mask").unwrap()[row] == 1 {
            assert!(seen.insert(key.to_vec()), "extra 语义键碰撞：{key:?}");
            assert_ne!(key, &[5, 1, 4103, 0], "实体引用不得通过 raw.u64_value 泄漏原编号");
        }
    }
}

#[test]
fn clone_plan_leaves_use_template_owner_and_zero_ordinal() {
    let mut state = review_state();
    state.entities[0].template.clone_build = Some(review_plan());
    let template = state.entities[0].template.clone();
    state.template_slots.push(review_blueprint(0, template));
    let batch = encoder().encode(&state).unwrap();
    for owner in [0, 2] {
        assert_eq!(review_raw(&batch, [3, owner, 3, 0]), 0, "Some 计划的零 mask 不能省略");
        for (field, value) in [(4, 0.0), (5, 7.0), (6, 9.0), (7, 0.0)] {
            let row = review_extra_row(&batch, [3, owner, field, 0]);
            assert_eq!(batch.f32_all("extra_num").unwrap()[row], normalize_fitted(value, 4.0, 64.0) as f32);
            assert_eq!(batch.i32_all("extra_ref").unwrap()[row], -1);
            assert_eq!(&batch.u32_all("extra_bits").unwrap()[2 * row..2 * row + 2], &[0, 0]);
        }
    }
}

#[test]
fn raw_state_rows_and_signed_bits_are_exact() {
    use crate::runtime::model_state::PoisonPayload;
    let mut state = review_state();
    let target = state.entities[0].id.0;
    for bits in [(-0.0f64).to_bits(), 123.5f64.to_bits()] {
        state.entities[0].states.push(review_entry(
            ModelPayload {
                kind: "poison".to_owned(),
                poison: Some(PoisonPayload { caster: None, target: Some(target), atp_bits: bits, count: 0 }),
                ..ModelPayload::default()
            },
            (1u64 << 53) + 1,
        ));
    }
    let mut entry = review_entry(ModelPayload { kind: "none".to_owned(), ..ModelPayload::default() }, 0);
    entry.extension_state_id = Some(0);
    state.entities[1].states.push(entry);
    state.entities[0].runtime.at_boost_millionths = i64::MIN;
    state.entities[1].runtime.at_boost_millionths = -1;
    state.entities[0].template.at_boost_millionths = -123;
    state.world.round_pos = -1;
    let batch = encoder().encode(&state).unwrap();
    assert_eq!(&batch.i32_all("state_entity").unwrap()[..3], &[0, 0, 1]);
    for row in 0..3 {
        assert_eq!(review_raw(&batch, [4, row, 4099, 0]), 1);
    }
    assert_eq!(review_raw(&batch, [4, 0, 4119, 0]), (-0.0f64).to_bits());
    assert_eq!(review_raw(&batch, [4, 1, 4119, 0]), 123.5f64.to_bits());
    assert_eq!(review_raw(&batch, [4, 2, 4100, 0]), 0);
    assert_eq!(review_raw(&batch, [2, 0, 4105, 0]), i64::MIN as u64);
    assert_eq!(review_raw(&batch, [2, 1, 4105, 0]), u64::MAX);
    assert_eq!(review_raw(&batch, [3, 0, 4113, 0]), (-123i64) as u64);
    assert_eq!(review_raw(&batch, [1, 0, 4121, 0]), u64::MAX);
    assert_eq!(&batch.u8_all("state_ref_present").unwrap()[..2], &[0, 1]);
    assert_eq!(&batch.i32_all("state_ref").unwrap()[..2], &[-1, 0]);
}

#[test]
fn lane_indices_are_not_fixed_keys_and_deferred_joins_runtime_rank_domain() {
    use crate::runtime::model_state::ModelDeferredSkill;
    let mut state = review_state();
    // 两个模板刻意使用不同 key；槽内蓝图复用实体 0 的 key，验证 key 是跨模板关系键。
    state.entities[1].template.skills.lanes[1].fixed_lane_key = 22;
    state.entities[0].states.push(review_entry(ModelPayload { kind: "none".to_owned(), ..ModelPayload::default() }, 10));
    state.entities[0].state_registration_cursor = 20;
    state.entities[0].template.skills.post_action_after_states.push(ModelDeferredSkill { state_cursor: 12, fixed_lane: 1 });
    state.entities[0].runtime.assassinate = Some(crate::runtime::entity::AssassinateRuntime {
        fixed_lane: 1,
        target: state.entities[1].id,
        break_on_damage: true,
    });
    let template = state.entities[0].template.clone();
    state.template_slots.push(review_blueprint(0, template));
    let batch = encoder().encode(&state).unwrap();
    let reference = review_extra_row(&batch, [2, 0, 1, 0]);
    assert_eq!(batch.i32_all("extra_ref").unwrap()[reference], 1, "暗杀保存的是下标 1，不是 key=1");
    let mut deferred = 0;
    let mut active = Vec::new();
    for (row, record) in batch.i32_all("list_index").unwrap().chunks_exact(5).enumerate() {
        if batch.u8_all("list_mask").unwrap()[row] == 0 { continue; }
        let field = record[2];
        if (256..=261).contains(&field) {
            assert_eq!(record[0], 3, "lane 列表 owner 必须属于 template 域");
            assert_eq!(batch.i32_all("lane_template").unwrap()[record[4] as usize], record[1]);
        }
        if field == 258 && record[1] == 0 { active.push(record[4]); }
        if field == 261 {
            deferred += 1;
            assert_eq!(batch.u8_all("order_key_present").unwrap()[row], 1);
            assert_eq!(&batch.u32_all("order_key").unwrap()[row * 2..row * 2 + 2], &[12, 0]);
            let rank_present = batch.u8_all("order_rank_present").unwrap()[row];
            if record[1] == 0 {
                assert_eq!(rank_present, 1);
                assert_eq!(batch.f32_all("order_rank").unwrap()[row], 0.5, "共享域为 10、12、20");
            } else {
                assert_eq!(record[1], 2);
                assert_eq!(rank_present, 0, "蓝图没有当前实体比较域");
            }
        }
    }
    assert_eq!(deferred, 2);
    assert_eq!(active, vec![1, 0, 1], "执行顺序与重复项不能丢失");
    let lane_keys = batch.i32_all("lane_key").unwrap();
    assert_eq!(&lane_keys[..6], &[88, 11, 88, 22, 88, 11]);
    assert_eq!(lane_keys[0], lane_keys[2], "相等 fixed key 必须保留相等关系");
    assert_ne!(lane_keys[1], lane_keys[3], "不同 fixed key 不得被模板内序号合并");
    assert_eq!(batch.u8_all("lane_mask").unwrap()[1], 1);
    assert_eq!(&batch.u8_all("lane_num_present").unwrap()[4..8], &[1; 4]);
    assert_eq!(&batch.f32_all("lane_num").unwrap()[4..8], &[0.0; 4]);
    assert_eq!(batch.u8_all("list_mask").unwrap().iter().filter(|value| **value == 1).count(), CapacityMeasure::measure(&state).v);
}

#[test]
fn out_of_template_lane_references_and_reserved_skills_are_rejected() {
    for case in 0..6 {
        let mut state = review_state();
        let target = state.entities[1].id;
        match case {
            0 => state.entities[0].template.skills.merge_lane_order.push(2),
            1 => state.entities[0].template.skills.active_order.push(2),
            2 => state.entities[0].template.skills.pre_action_order.push(2),
            3 => state.entities[0].template.skills.post_damage_order.push(2),
            4 => state.entities[0].template.skills.post_action_after_states.push(crate::runtime::model_state::ModelDeferredSkill { state_cursor: 0, fixed_lane: 2 }),
            _ => state.entities[0].runtime.assassinate = Some(crate::runtime::entity::AssassinateRuntime {
                fixed_lane: 2,
                target,
                break_on_damage: false,
            }),
        }
        assert!(matches!(encoder().encode(&state), Err(EncodeError::InvalidReference { .. })), "case={case}");
    }
    for skill in [0, 43, 50, 51, u32::MAX] {
        let mut state = review_state();
        state.entities[0].template.skills.lanes[0].skill_id = skill;
        assert!(matches!(encoder().encode(&state), Err(EncodeError::UnknownCategory { .. })), "skill={skill}");
    }
}

#[test]
fn late_extra_failure_clears_every_tensor_and_preserves_batch_neighbors() {
    let mut state = review_state();
    state.entities[0].template.clone_build = Some(review_plan());
    state.entities[0].slots.push(counter_slot(u64::MAX));
    let template = state.entities[0].template.clone();
    state.template_slots.push(review_blueprint(0, template));
    state.entities[0].states.push(review_entry(ModelPayload { kind: "none".to_owned(), ..ModelPayload::default() }, (1u64 << 53) + 1));
    let encoder = encoder();
    let expected = encoder.encode(&state).unwrap();
    let padding = EncodedBatch::new(encoder.profile(), 1);
    let mut batch = EncodedBatch::new(encoder.profile(), 3);
    for slot in [2, 0, 1] { encoder.encode_into(&state, slot, &mut batch).unwrap(); }
    for spec in TENSOR_SPECS {
        let single = tensor_bytes(&expected, spec.name);
        let all = tensor_bytes(&batch, spec.name);
        for slot in 0..3 {
            assert_eq!(&all[slot * single.len()..(slot + 1) * single.len()], single.as_slice(), "{}", spec.name);
        }
    }
    let before: Vec<_> = TENSOR_SPECS.iter().map(|spec| tensor_bytes(&batch, spec.name)).collect();
    let mut invalid = state.clone();
    invalid.entities[0].runtime.assassinate = Some(crate::runtime::entity::AssassinateRuntime {
        fixed_lane: 2,
        target: invalid.entities[1].id,
        break_on_damage: true,
    });
    assert!(matches!(encoder.encode_into(&invalid, 1, &mut batch), Err(EncodeError::InvalidReference { .. })));
    for (spec, previous) in TENSOR_SPECS.iter().zip(&before) {
        let empty = tensor_bytes(&padding, spec.name);
        let stride = empty.len();
        let after = tensor_bytes(&batch, spec.name);
        assert_eq!(&after[..stride], &previous[..stride], "{} 左邻居", spec.name);
        assert_eq!(&after[stride..2 * stride], empty.as_slice(), "{} 失败清理", spec.name);
        assert_eq!(&after[2 * stride..], &previous[2 * stride..], "{} 右邻居", spec.name);
    }
    encoder.encode_into(&state, 1, &mut batch).unwrap();
    for (spec, previous) in TENSOR_SPECS.iter().zip(before) {
        assert_eq!(tensor_bytes(&batch, spec.name), previous, "{} 失败后复用", spec.name);
    }
    let actual = expected.u8_all("extra_mask").unwrap().iter().filter(|value| **value == 1).count();
    assert!(actual <= CapacityMeasure::measure(&state).x);
}

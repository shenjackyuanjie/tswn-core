use super::*;

#[test]
fn encodes_global_entity_and_template_families() {
    let state = battle_state(12);
    let batch = encoder().encode(&state).unwrap();
    let mask = batch.u8_all("entity_mask").unwrap();
    assert_eq!(mask[..state.entities.len()], vec![1; state.entities.len()][..]);
    assert!(
        mask[state.entities.len()..].iter().all(|value| *value == 0),
        "padding 行必须为 0"
    );
    let template_mask = batch.u8_all("template_mask").unwrap();
    let measure = CapacityMeasure::measure(&state);
    assert_eq!(template_mask[..measure.h], vec![1; measure.h][..]);
    assert!(template_mask[measure.h..].iter().all(|value| *value == 0));
    // global_num[4] 是 legacy_step_scheduler 布尔；[5] 是固定计数尺度。
    let global = batch.f32_all("global_num").unwrap();
    assert_eq!(global[4], f32::from(state.legacy_step_scheduler));
    assert!((global[5] - state.entities.len() as f32 / 15.0).abs() < 1e-6);
    // team_mask 覆盖全部输入队伍。
    let team_mask = batch.u8_all("team_mask").unwrap();
    assert_eq!(team_mask[..state.input_teams.len()], vec![1; state.input_teams.len()][..]);
    assert!(team_mask[state.input_teams.len()..].iter().all(|value| *value == 0));
}

#[test]
fn encodes_lane_rows_and_boost_presence() {
    let state = battle_state(2);
    let batch = encoder().encode(&state).unwrap();
    let measure = CapacityMeasure::measure(&state);
    assert_eq!(batch.u8_all("lane_mask").unwrap()[..measure.l], vec![1; measure.l][..]);
    assert!(batch.u8_all("lane_mask").unwrap()[measure.l..].iter().all(|value| *value == 0));
    let templates = batch.i32_all("lane_template").unwrap();
    assert!(templates[..measure.l].iter().all(|row| *row >= 0));
    let presence = batch.u8_all("lane_num_present").unwrap();
    for row in 0..measure.l {
        assert_eq!(&presence[row * 4..row * 4 + 2], &[1, 1]);
    }
    let list = batch.i32_all("list_index").unwrap();
    let list_mask = batch.u8_all("list_mask").unwrap();
    let list_count = list_mask.iter().position(|value| *value == 0).unwrap_or(list_mask.len());
    assert!(list_mask[..list_count].iter().all(|value| *value == 1));
    assert!(list_mask[list_count..].iter().all(|value| *value == 0));
    for row in 0..list_count {
        assert!(list[row * 5 + 4] >= 0);
        if (256..=261).contains(&list[row * 5 + 2]) {
            assert_eq!(list[row * 5], 3);
        }
    }
}

#[test]
fn state_categories_use_frozen_vocabularies() {
    let mut state = battle_state(2);
    let row = state.entities[0].states.len();
    state.entities[0].states.push(crate::runtime::model_state::ModelStateEntry {
        legacy_order_key: 1,
        extension_state_id: Some(0),
        hook_mask: 0,
        priority: 0,
        registration_order: 1,
        runtime_registration_order: 1,
        payload: ModelPayload {
            kind: "none".to_owned(),
            ..ModelPayload::default()
        },
    });
    let batch = encoder().encode(&state).unwrap();
    let cats = batch.i32_all("state_cat").unwrap();
    let presence = batch.u8_all("state_cat_present").unwrap();
    assert!(cats[row * 2] > 0);
    assert!(cats[row * 2 + 1] > 0, "Some(0) 也必须映射为正分类");
    assert_eq!(&presence[row * 2..row * 2 + 2], &[1, 1]);

    state.entities[0].states[row].extension_state_id = None;
    let batch = encoder().encode(&state).unwrap();
    let cats = batch.i32_all("state_cat").unwrap();
    let presence = batch.u8_all("state_cat_present").unwrap();
    assert_eq!(cats[row * 2 + 1], 0);
    assert_eq!(&presence[row * 2..row * 2 + 2], &[1, 0]);
}

#[test]
fn order_lists_preserve_keys_and_rank_domains() {
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
    let template = state
        .entities
        .iter_mut()
        .find(|entity| !entity.template.skills.lanes.is_empty())
        .expect("测试战斗必须有至少一条 lane");
    template
        .template
        .skills
        .post_action_after_states
        .push(crate::runtime::model_state::ModelDeferredSkill {
            state_cursor: 17,
            fixed_lane: 0,
        });

    let batch = encoder().encode(&state).unwrap();
    let indices = batch.i32_all("list_index").unwrap();
    let mask = batch.u8_all("list_mask").unwrap();
    let order_present = batch.u8_all("order_key_present").unwrap();
    let rank_present = batch.u8_all("order_rank_present").unwrap();
    let mut seen = [false; 4];
    let count = mask.iter().position(|value| *value == 0).unwrap_or(mask.len());
    for row in 0..count {
        let field_class = indices[row * 5 + 2];
        let slot = match field_class {
            261 => Some(0),
            270 => Some(1),
            271 => Some(2),
            272 => Some(3),
            _ => None,
        };
        let Some(slot) = slot else { continue };
        seen[slot] = true;
        assert_eq!(order_present[row], 1, "field_class={field_class} 必须有精确顺序键");
        let has_entity_domain = field_class != 261 || (indices[row * 5 + 1] as usize) < state.entities.len();
        assert_eq!(rank_present[row], u8::from(has_entity_domain), "field_class={field_class}");
    }
    assert!(seen.into_iter().all(|present| present), "四类顺序记录都应出现");
}

#[test]
fn option_absence_is_distinct_from_zero() {
    let mut state = battle_state(2);
    state.entities[0].runtime.protect_pre_defend_skill_count = Some(0);
    let batch = encoder().encode(&state).unwrap();
    assert_eq!(batch.u8_all("entity_num_present").unwrap()[14], 1, "Some(0) 仍是存在");
    assert_eq!(batch.f32_all("entity_num").unwrap()[14], 0.0);

    state.entities[0].runtime.protect_pre_defend_skill_count = None;
    let batch = encoder().encode(&state).unwrap();
    assert_eq!(batch.u8_all("entity_num_present").unwrap()[14], 0, "None 必须 presence=0");
    assert_eq!(batch.f32_all("entity_num").unwrap()[14], 0.0);
}

#[test]
fn hide_presence_drives_numeric_slots() {
    let mut state = battle_state(2);
    state.entities[0].runtime.hide = Some(crate::runtime::entity::HideRuntime {
        level: 3,
        attract_bits: 2.5f64.to_bits(),
        agility: 7,
        defense: 8,
        resistance: 9,
    });
    let batch = encoder().encode(&state).unwrap();
    assert_eq!(batch.u8_all("entity_bool").unwrap()[7], 1, "hide 整体 presence");
    assert_eq!(&batch.u8_all("entity_num_present").unwrap()[19..24], &[1, 1, 1, 1, 1]);
    let numbers = batch.f32_all("entity_num").unwrap();
    assert_eq!(numbers[19], normalize_fitted(3.0, 4.0, 64.0) as f32);
    assert_eq!(numbers[21], normalize_fitted(7.0, 4.0, 64.0) as f32);
    assert_eq!(numbers[22], normalize_fitted(8.0, 4.0, 64.0) as f32);
    assert_eq!(numbers[23], normalize_fitted(9.0, 4.0, 64.0) as f32);
    // attract 走 f64 还原后归一化（s_f=4、c_f=64）。
    let expected = normalize_fitted(2.5, 4.0, 64.0) as f32;
    assert!((numbers[20] - expected).abs() < 1e-6);

    state.entities[0].runtime.hide = None;
    let batch = encoder().encode(&state).unwrap();
    assert_eq!(batch.u8_all("entity_bool").unwrap()[7], 0);
    assert_eq!(&batch.u8_all("entity_num_present").unwrap()[19..24], &[0, 0, 0, 0, 0]);
    assert_eq!(&batch.f32_all("entity_num").unwrap()[19..24], &[0.0; 5]);
}

#[test]
fn optional_references_use_minus_one_without_presence() {
    let mut state = battle_state(2);
    state.entities[0].runtime.protect_to = None;
    state.entities[0].runtime.counter.last_target = None;
    state.entities[0].runtime.assassinate = None;
    let batch = encoder().encode(&state).unwrap();
    let refs = batch.i32_all("entity_ref").unwrap();
    let presence = batch.u8_all("entity_ref_present").unwrap();
    assert_eq!(refs[2], -1, "缺失引用必须是 -1");
    assert_eq!(&presence[2..5], &[0, 0, 0]);
    assert!(refs[0] >= 0 && refs[1] >= 0, "owner/root_owner 必填");
    assert_eq!(&presence[0..2], &[1, 1]);

    let target = state.entities[1].id;
    state.entities[0].runtime.protect_to = Some(target);
    state.entities[0].runtime.assassinate = Some(crate::runtime::entity::AssassinateRuntime {
        fixed_lane: 0,
        target,
        break_on_damage: true,
    });
    let batch = encoder().encode(&state).unwrap();
    let refs = batch.i32_all("entity_ref").unwrap();
    let presence = batch.u8_all("entity_ref_present").unwrap();
    assert_eq!(refs[2], 1, "protect_to 指向实体 1 的稠密行");
    assert_eq!(presence[2], 1);
    assert_eq!(refs[4], 1);
    assert_eq!(presence[4], 1);
}

#[test]
fn team_mask_ignores_alive_and_alive_group_count() {
    let mut state = battle_state(2);
    for entity in &mut state.entities {
        if entity.input_team_index == 0 {
            entity.runtime.alive = false;
        }
    }
    state.world.flat_alive.clear();
    state.world.team_alive = vec![Vec::new(); state.world.team_alive.len()];
    state.world.alive_group_count = 0;
    let batch = encoder().encode(&state).unwrap();
    let team_mask = batch.u8_all("team_mask").unwrap();
    assert_eq!(team_mask[0], 1, "零存活队伍仍是输入标签位置");
    assert_eq!(team_mask[1], 1);
    // alive_group_count 只作机制特征进入 global_num[2]，不作 mask。
    assert_eq!(batch.f32_all("global_num").unwrap()[2], 0.0);
}

#[test]
fn clone_build_presence_controls_template_slots() {
    let mut state = battle_state(4);
    // 先清零前两个实体模板，保证与具体名字无关；再只给第一个挂合成 clone_build。
    for entity in state.entities.iter_mut().take(2) {
        entity.template.clone_build = None;
    }
    let batch = encoder().encode(&state).unwrap();
    assert_eq!(batch.u8_all("template_bool").unwrap()[4], 0);
    // `[0..16]` 跟随模板行（含 move_state 槽 16），`[17..30]` 跟随 clone_build。
    let presence = batch.u8_all("template_num_present").unwrap();
    assert_eq!(&presence[..17], &[1; 17][..]);
    assert_eq!(&presence[17..31], &[0; 14]);

    state.entities[0].template.clone_build = Some(crate::runtime::entity::CloneBuildData {
        attrs: [1; 8],
        weapon_attr_bonus: [0; 8],
        name_factor_bits: 0.5f64.to_bits(),
        child_name_factor_bits: 0.25f64.to_bits(),
        adjustments: crate::runtime::entity::CloneStatAdjustments {
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
        score_skill_boost_plan: None,
    });
    let batch = encoder().encode(&state).unwrap();
    let bools = batch.u8_all("template_bool").unwrap();
    assert_eq!(bools[4], 1, "clone 整体 presence");
    let presence = batch.u8_all("template_num_present").unwrap();
    assert_eq!(&presence[..17], &[1; 17][..]);
    assert_eq!(&presence[17..31], &[1; 14][..], "`[17..30]` 跟随 clone_build");
    let numbers = batch.f32_all("template_num").unwrap();
    assert_eq!(numbers[17], normalize_fitted(0.5, 4.0, 64.0) as f32);
    assert_eq!(numbers[18], normalize_fitted(0.25, 4.0, 64.0) as f32);
    // 第二个模板仍无 clone_build（template_bool 每行 5 个元素）。
    assert_eq!(bools[5 + 4], 0);
    assert_eq!(&presence[31 + 17..31 + 31], &[0; 14][..]);
    assert_eq!(&presence[31..31 + 17], &[1; 17][..]);
    // clone 数组在整体存在时写入，缺失时为 0。
    let attrs = batch.u32_all("template_clone_attr").unwrap();
    assert_eq!(&attrs[..8], &[1; 8][..]);
    assert_eq!(&attrs[8..16], &[0; 8][..]);
}

#[test]
fn slot_and_extra_families_route_values_and_raw_bits() {
    let mut state = battle_state(2);
    state.entities[0].slots.push(counter_slot(u64::MAX));
    let batch = encoder().encode(&state).unwrap();
    let q = batch.u8_all("slot_mask").unwrap().iter().filter(|value| **value == 1).count();
    assert_eq!(q, state.entities.iter().map(|entity| entity.slots.len()).sum::<usize>());
    let values = batch.u8_all("slot_value_present").unwrap();
    assert!(values.iter().any(|value| *value == 1));
    let fields = batch.i32_all("extra_index").unwrap();
    let masks = batch.u8_all("extra_mask").unwrap();
    let mut found_raw_counter = false;
    for row in 0..masks.len() {
        if masks[row] == 1 && fields[row * 4 + 2] == 4103 {
            let bits = &batch.u32_all("extra_bits").unwrap()[row * 2..row * 2 + 2];
            assert_eq!(bits, &[u32::MAX, u32::MAX]);
            found_raw_counter = true;
        }
    }
    assert!(found_raw_counter, "计数槽必须保留 raw.u64_value");
}

// ------------------------------------------------------------ 引用与置换

#[test]
fn references_are_remapped_to_dense_rows_without_leaking_entity_idx() {
    let mut state = battle_state(4);
    // 稀疏 EntityIdx（带空洞），原编号远大于 E_max。
    let mut renames: BTreeMap<u32, u32> = BTreeMap::new();
    for (index, entity) in state.entities.iter().enumerate() {
        renames.insert(entity.id.0, 500 + index as u32 * 7);
    }
    state.entity_slot_count = state.entity_slot_count.max(*renames.values().max().unwrap() as usize + 1);
    for entity in &mut state.entities {
        entity.id = EntityIdx(renames[&entity.id.0]);
        entity.runtime.owner = EntityIdx(renames[&entity.runtime.owner.0]);
        entity.runtime.root_owner = EntityIdx(renames[&entity.runtime.root_owner.0]);
        if let Some(id) = entity.runtime.protect_to {
            entity.runtime.protect_to = Some(EntityIdx(renames[&id.0]));
        }
        if let Some(id) = entity.runtime.counter.last_target {
            entity.runtime.counter.last_target = Some(EntityIdx(renames[&id.0]));
        }
    }
    for team in &mut state.input_teams {
        for id in team.iter_mut() {
            *id = EntityIdx(renames[&id.0]);
        }
    }
    for list in [&mut state.world.round_order, &mut state.world.flat_alive] {
        for id in list.iter_mut() {
            *id = EntityIdx(renames[&id.0]);
        }
    }
    for roster in state.world.team_roster.iter_mut().chain(state.world.team_alive.iter_mut()) {
        for id in roster.iter_mut() {
            *id = EntityIdx(renames[&id.0]);
        }
    }
    let batch = encoder().encode(&state).unwrap();
    let rows = state.entities.len();
    for value in &batch.i32_all("entity_ref").unwrap()[..5 * rows] {
        if *value >= 0 {
            assert!(*value < rows as i32, "引用必须是稠密行");
        }
    }
    // 原始 EntityIdx 不得出现在关系值中；索引张量允许冻结的 field_class 编号。
    for spec in TENSOR_SPECS {
        if spec.dtype != Dtype::I32 {
            continue;
        }
        if spec.name == "extra_index" {
            continue;
        }
        for value in batch.i32_all(spec.name).unwrap() {
            assert!(*value < 500 || *value == -1, "{} 泄漏了原始编号 {value}", spec.name);
        }
    }
}

#[test]
fn entity_storage_permutation_is_equivariant() {
    let state = battle_state(6);
    let encoder = encoder();
    let baseline = encoder.encode(&state).unwrap();
    let moved = encoder.encode(&permuted_entities(&state)).unwrap();
    let rows = state.entities.len();
    for row in 0..rows {
        let source = rows - 1 - row;
        assert_eq!(
            baseline.u8_all("entity_mask").unwrap()[source],
            moved.u8_all("entity_mask").unwrap()[row]
        );
        let base_ref = &baseline.i32_all("entity_ref").unwrap()[source * 5..source * 5 + 5];
        let moved_ref = &moved.i32_all("entity_ref").unwrap()[row * 5..row * 5 + 5];
        for (left, right) in base_ref.iter().zip(moved_ref) {
            let expected = if *left < 0 { -1 } else { rows as i32 - 1 - *left };
            assert_eq!(*right, expected, "引用必须随置换同步");
        }
        assert_eq!(
            &baseline.f32_all("entity_num").unwrap()[source * 24..source * 24 + 24],
            &moved.f32_all("entity_num").unwrap()[row * 24..row * 24 + 24],
            "数值槽必须随置换同步"
        );
    }
}

// ------------------------------------------------------------ 批布局与缓冲复用

#[test]
fn skill_bearing_battles_encode_end_to_end() {
    // 覆盖火焰/治疗/幻影/毒/冰/急速/魅惑/诅咒/保护/强化/反射/铁壁/分身/召唤/复活等技能，
    // 让状态载荷、幻影与蓝图在真实对局里被编码。
    let pools = [
        "mario@red+fire\nluigi@red+heal\n\npeach@blue+shadow\nbowser@blue+poison\nseed:s1",
        "a@red+ice\nb@red+haste\n\nc@blue+charm\nd@blue+curse\nseed:s2",
        "e@red+protect\nf@red+upgrade\n\ng@blue+reflect\nh@blue+iron\nseed:s3",
        "i@red+clone\nj@red+shadow\n\nk@blue+summon\nl@blue+revive\nseed:s4",
        "m@red+charge\nn@red+accumulate\n\no@blue+hide\np@blue+assassinate\nseed:s5",
    ];
    let encoder = encoder();
    let mut frames = 0;
    for pool in pools {
        let mut runner = RuntimeRunner::new_from_namerena_raw(pool.to_owned()).unwrap();
        for _ in 0..60 {
            if runner.have_winner() {
                break;
            }
            let state = runner.model_state().unwrap();
            let batch = encoder.encode(&state).unwrap_or_else(|error| panic!("{pool} 编码失败：{error}"));
            // 每个被编码的样本，实体行与模板行都必须与容量计费口径一致。
            let measure = CapacityMeasure::measure(&state);
            let mask = batch.u8_all("entity_mask").unwrap();
            assert_eq!(&mask[..state.entities.len()], &vec![1; state.entities.len()][..]);
            assert_eq!(&batch.u8_all("template_mask").unwrap()[..measure.h], &vec![1; measure.h][..]);
            frames += 1;
            runner.main_round();
        }
        // 终局帧带 winner_team，必须被 AlreadyDecided 拒绝（结果泄漏门禁）。
        let terminal = runner.model_state().unwrap();
        assert!(terminal.world.winner_team.is_some(), "{pool} 应已产生胜者");
        assert!(matches!(
            encoder.encode(&terminal).unwrap_err(),
            EncodeError::AlreadyDecided { .. }
        ));
    }
    assert!(frames > 100, "样本过少（{frames}），冒烟覆盖不足");
}

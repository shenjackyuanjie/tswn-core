//! 编码行为回归的共享状态、校准与字节比较助手。

use super::*;
use crate::encoder::batch::{Dtype, TENSOR_SPECS};
use crate::encoder::capacity::BASELINE_64;
use crate::encoder::manifest::{CALIBRATION_SCHEMA, CALIBRATION_SCHEMA_VERSION, CalibrationFieldReport, CalibrationReport};
use crate::runtime::RuntimeRunner;

fn runner() -> RuntimeRunner {
    RuntimeRunner::new_from_namerena_raw("private-left@red\n\nprivate-right@blue\nseed:private-seed".into()).unwrap()
}

/// 跑若干回合后的状态；召唤/分身类 build 会带来更多实体与蓝图。
fn battle_state(rounds: usize) -> BattleModelState {
    let mut runner = runner();
    for _ in 0..rounds {
        if runner.have_winner() {
            break;
        }
        runner.main_round();
    }
    runner.model_state().unwrap()
}

fn calibration_field() -> CalibrationFieldReport {
    CalibrationFieldReport {
        count: 100,
        min: 0.0,
        max: 100.0,
        p50: 10.0,
        p99: 64.0,
        abs_p50: 10.0,
        abs_p99: 64.0,
        s_f: 4.0,
        c_f: 64.0,
    }
}

/// 合成覆盖全部数值槽的报告，包含真实采集器也会输出的固定计数统计。
fn manifest() -> EncoderManifest {
    let mut fields = BTreeMap::new();
    for (path, _) in required_normalization() {
        fields.insert(path.to_owned(), calibration_field());
    }
    let report = CalibrationReport {
        schema: CALIBRATION_SCHEMA.to_owned(),
        schema_version: CALIBRATION_SCHEMA_VERSION,
        state_schema_version: MODEL_STATE_SCHEMA_VERSION,
        split: "train".to_owned(),
        require_label: true,
        samples_seen: 100,
        samples_selected: 100,
        input_sha256: "aa".to_owned(),
        executable_sha256: "bb".to_owned(),
        selected_rows_digest: "cc".to_owned(),
        calibrator: "test-calibrator".to_owned(),
        fields,
    };
    EncoderManifest::from_calibration(&report, &BASELINE_64).unwrap()
}

fn encoder() -> FeatureEncoder { FeatureEncoder::new(manifest()).expect("测试 manifest 必须有效") }

/// `core.entity.minion_counter` 槽（U64 计数语义）。
fn counter_slot(value: u64) -> crate::runtime::model_state::ModelSlot {
    crate::runtime::model_state::ModelSlot {
        slot_id: 5,
        bool_value: None,
        i64_value: None,
        u64_value: Some(value),
        template: None,
    }
}

/// 复制实体并分配新 id；引用仍指向原实体，因此状态保持自洽。
fn replicate_entities(state: &BattleModelState, extra: usize) -> BattleModelState {
    let mut cloned = state.clone();
    let base = cloned.entities.len();
    for index in 0..extra {
        let mut entity = cloned.entities[index % base].clone();
        entity.id = EntityIdx(10_000 + index as u32);
        cloned.entity_slot_count = cloned.entity_slot_count.max(entity.id.0 as usize + 1);
        cloned.entities.push(entity);
    }
    cloned
}

/// 重排实体存储顺序（世界列表与引用不变，由 encoder 按 EntityIdx 解引用）。
fn permuted_entities(state: &BattleModelState) -> BattleModelState {
    let mut cloned = state.clone();
    cloned.entities.reverse();
    cloned
}

/// 按 dtype 取出整段字节，用于逐字节比较。
fn tensor_bytes(batch: &EncodedBatch, name: &str) -> Vec<u8> {
    let spec = EncodedBatch::spec(name).unwrap();
    match spec.dtype {
        Dtype::F32 => slice_as_bytes(batch.f32_all(name).unwrap()).to_vec(),
        Dtype::I32 => slice_as_bytes(batch.i32_all(name).unwrap()).to_vec(),
        Dtype::U8 => batch.u8_all(name).unwrap().to_vec(),
        Dtype::U32 => slice_as_bytes(batch.u32_all(name).unwrap()).to_vec(),
    }
}

fn slice_as_bytes<T>(values: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values)) }
}

// ------------------------------------------------------------ 基本路径

mod batching;
mod families;
mod review;
mod validation;

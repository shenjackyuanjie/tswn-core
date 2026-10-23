//! 维护前后公开 API 的单线程对照基准，默认依次运行四种固定输入的十六项工作负载。
//!
//! encoder 从固定帧边界准备一份未决机制状态，反复调用 encode_into 并复用批缓冲；
//! 同时测量 case-sized 紧凑容量与 baseline-64 标准容量，batch_size 都为 1；
//! 紧凑档位用于观察重复容量计量的局部影响，标准档位保留真实 padding 成本，不单凭紧凑结果推断收益。
//! 校准常数是固定的合成数据，仅用于版本间对照，不是训练或推理工件。
//! BattleSession 每次从构造运行到终止并读取结果，分别关闭和开启图标，覆盖会话内缓存生命周期。
//!
//! 输入读取、状态准备、manifest 构造、批分配、预热和 JSON 序列化都在计时之外。
//! 计时内包含被测 API、逐帧消费、对象释放及循环控制；checksum 在计时后计算，
//! encoder 覆盖最后一次全部张量，session 覆盖最后一次终局结果，不逐帧序列化或哈希。
//! 每行 JSON 独立报告 ops、elapsed_seconds、checksum、帧数和错误；checksum 不含迭代次数。
//! 默认每项编码 20,000 次、每项会话 100 次，不按耗时自适应改变次数；两版本须使用相同参数。
//! 本示例不测校准质量、编码缓冲分配性能、跨会话缓存或并行吞吐，总耗时由固定次数和单次操作成本决定。

use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use serde_json::{Value, json};
use tswn_core::cli_api::battle::{BattleModelSession, BattleOptions, BattleResult, BattleSession};
use tswn_core::encoder::batch::{EncodedBatch, TENSOR_SPECS};
use tswn_core::encoder::capacity::{BASELINE_64, CapacityMeasure, EncoderProfile};
use tswn_core::encoder::encode::{FeatureEncoder, required_normalization};
use tswn_core::encoder::manifest::{
    CALIBRATION_SCHEMA, CALIBRATION_SCHEMA_VERSION, CalibrationFieldReport, CalibrationReport, EncoderManifest,
};
use tswn_core::runtime::model_state::{BattleModelState, MODEL_STATE_SCHEMA_VERSION};

const CASES: [(&str, &str); 4] = [
    ("1v1", "1v1-0f92cb76cc37fdc5.txt"),
    ("2v2", "2v2-554f4128af707167.txt"),
    ("ffa8", "ffa_8-16d11de1ebe1df41.txt"),
    ("3v3v3", "3v3v3-0ace5df17b84e26a.txt"),
];

#[derive(Parser)]
struct Args {
    /// 固定输入所在目录；默认使用本仓库 tswn_test/cases/runtime_stress。
    #[arg(long)]
    input_dir: Option<PathBuf>,
    /// 每个容量档位、每个 fixture 的固定编码次数，两版本使用相同值。
    #[arg(long, default_value_t = 20_000, value_parser = clap::value_parser!(u32).range(1..))]
    encoder_iters: u32,
    /// 每个图标开关、每个 fixture 的固定会话次数，两版本使用相同值。
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..))]
    session_iters: u32,
    /// 每项工作负载在计时前完整执行的次数，不计入 ops。
    #[arg(long, default_value_t = 3)]
    warmup: u32,
    /// 每场会话的实际回合预算；到达预算的正常截断不算错误。
    #[arg(long, default_value_t = 256, value_parser = clap::value_parser!(u32).range(1..))]
    max_rounds: u32,
    /// encoder 状态最多预推进的可见帧数；若提前决出胜者，使用上一份未决状态。
    #[arg(long, default_value_t = 4)]
    state_frames: u32,
}

struct Measurement<T> {
    ops: u64,
    elapsed_seconds: f64,
    errors: u64,
    error: Option<String>,
    last: Option<T>,
}

/// 预热不计时；首个 API 错误立即结束该项，仍返回可输出的错误记录。
fn measure<T>(args: &Args, iterations: u32, mut operation: impl FnMut() -> Result<T, String>) -> Measurement<T> {
    let mut measured = Measurement {
        ops: 0,
        elapsed_seconds: 0.0,
        errors: 0,
        error: None,
        last: None,
    };
    for _ in 0..args.warmup {
        match operation() {
            Ok(value) => {
                black_box(value);
            }
            Err(error) => {
                measured.errors = 1;
                measured.error = Some(format!("预热失败：{error}"));
                return measured;
            }
        }
    }
    let started = Instant::now();
    for _ in 0..iterations {
        measured.ops += 1;
        match operation() {
            Ok(value) => measured.last = Some(black_box(value)),
            Err(error) => {
                measured.errors = 1;
                measured.error = Some(error);
                break;
            }
        }
    }
    measured.elapsed_seconds = started.elapsed().as_secs_f64();
    measured
}

/// 固定的 FNV-1a 字节摘要，仅用于同一源码、输入和参数之间的确定性对照。
fn hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
    }
    hash
}

const HASH_START: u64 = 0xcbf2_9ce4_8422_2325;

fn record<T>(case: &str, workload: &str, input_hash: u64, measured: &Measurement<T>, checksum: Option<u64>) -> Value {
    json!({
        "case": case, "workload": workload, "input_checksum": format!("{input_hash:016x}"),
        "ops": measured.ops, "elapsed_seconds": measured.elapsed_seconds,
        "checksum": checksum.map(|value| format!("{value:016x}")),
        "errors": measured.errors, "error": measured.error,
    })
}

fn prepare_state(raw: &str, frames: u32) -> Result<BattleModelState, String> {
    let mut session = BattleModelSession::new(raw, BattleOptions::default()).map_err(|error| error.to_string())?;
    let mut state = session.model_state().map_err(|error| error.to_string())?;
    for _ in 0..frames {
        if session.next_frame().map_err(|error| error.to_string())?.is_none() {
            break;
        }
        let next = session.model_state().map_err(|error| error.to_string())?;
        if next.world.winner_team.is_some() {
            break;
        }
        state = next;
    }
    Ok(state)
}

/// 档位仅适配这一份固定状态，并满足公开 manifest 的 Q/S/V/X 容量不变量。
fn case_profile(state: &BattleModelState) -> EncoderProfile {
    let measured = CapacityMeasure::measure(state);
    let e = measured.e.max(1);
    let s = measured.s.max(e);
    let l = measured.l.max(1);
    let q = measured.q.max(7 * e + 3);
    let v = measured.v.max(5 * l);
    let h = measured.h.max(1);
    EncoderProfile {
        name: "maintenance-case-sized",
        e_max: e,
        t_max: measured.t.max(1),
        r_max: measured.r.max(1),
        h_max: h,
        l_max: l,
        s_max: s,
        q_max: q,
        v_max: v,
        x_max: measured.x.max(v + 15 * h + 9 * e + 3 * q + 3 * s + 2),
    }
}

fn make_encoder(profile: &EncoderProfile) -> Result<FeatureEncoder, String> {
    let fields = required_normalization()
        .into_iter()
        .map(|(path, _)| {
            (
                path.to_owned(),
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
                },
            )
        })
        .collect();
    let report = CalibrationReport {
        schema: CALIBRATION_SCHEMA.to_owned(),
        schema_version: CALIBRATION_SCHEMA_VERSION,
        state_schema_version: MODEL_STATE_SCHEMA_VERSION,
        split: "train".to_owned(),
        require_label: true,
        samples_seen: 100,
        samples_selected: 100,
        input_sha256: "00".repeat(32),
        executable_sha256: "00".repeat(32),
        selected_rows_digest: "00".repeat(32),
        calibrator: "maintenance-synthetic-only".to_owned(),
        fields,
    };
    let manifest = EncoderManifest::from_calibration(&report, profile).map_err(|error| error.to_string())?;
    FeatureEncoder::new(manifest).map_err(|error| error.to_string())
}

fn batch_checksum(batch: &EncodedBatch) -> Result<u64, String> {
    let mut hash = HASH_START;
    for spec in TENSOR_SPECS {
        let bytes = batch.tensor_bytes(spec.name).map_err(|error| error.to_string())?;
        hash = hash_bytes(hash, spec.name.as_bytes());
        hash = hash_bytes(hash, &(bytes.len() as u64).to_le_bytes());
        hash = hash_bytes(hash, &bytes);
    }
    Ok(hash)
}

fn play_session(raw: &str, include_icons: bool, max_rounds: u32) -> Result<BattleResult, String> {
    let options = BattleOptions {
        include_icons,
        max_rounds: max_rounds as usize,
        ..BattleOptions::default()
    };
    let mut session = BattleSession::new(black_box(raw), black_box(options)).map_err(|error| error.to_string())?;
    while let Some(frame) = session.next_frame().map_err(|error| error.to_string())? {
        black_box(frame);
    }
    session.result().ok_or_else(|| "会话结束后缺少结果".to_owned())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if !cfg!(feature = "png_render") {
        return Err("图标对照需要启用 png_render feature".into());
    }
    let input_dir = args
        .input_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tswn_test/cases/runtime_stress"));
    let mut failed = false;
    for (case, file) in CASES {
        let raw = std::fs::read_to_string(input_dir.join(file))?.replace("\r\n", "\n");
        let input_hash = hash_bytes(HASH_START, raw.as_bytes());
        let state = prepare_state(&raw, args.state_frames)?;
        for profile in [case_profile(&state), BASELINE_64] {
            let encoder = make_encoder(&profile)?;
            let mut batch = EncodedBatch::new(encoder.profile(), 1);
            let measured = measure(&args, args.encoder_iters, || {
                encoder
                    .encode_into(black_box(&state), 0, black_box(&mut batch))
                    .map_err(|error| error.to_string())?;
                black_box(&batch);
                Ok(())
            });
            let checksum = if measured.last.is_some() {
                Some(batch_checksum(&batch)?)
            } else {
                None
            };
            let mut row = record(case, "encoder", input_hash, &measured, checksum);
            row["profile_name"] = json!(profile.name);
            row["profile"] = serde_json::to_value(encoder.profile())?;
            row["batch_size"] = json!(1);
            row["state_round"] = json!(state.round);
            row["entities"] = json!(state.entities.len());
            row["warmup"] = json!(args.warmup);
            row["iterations"] = json!(args.encoder_iters);
            row["state_frames"] = json!(args.state_frames);
            failed |= measured.errors != 0;
            println!("{row}");
        }

        for include_icons in [false, true] {
            let measured = measure(&args, args.session_iters, || play_session(&raw, include_icons, args.max_rounds));
            let checksum = measured
                .last
                .as_ref()
                .map(serde_json::to_vec)
                .transpose()?
                .map(|bytes| hash_bytes(HASH_START, &bytes));
            let mut row = record(case, "battle_session", input_hash, &measured, checksum);
            row["include_icons"] = json!(include_icons);
            row["max_rounds"] = json!(args.max_rounds);
            row["warmup"] = json!(args.warmup);
            row["iterations"] = json!(args.session_iters);
            row["frames"] = json!(measured.last.as_ref().map(|result| result.frames_emitted));
            row["rounds"] = json!(measured.last.as_ref().map(|result| result.rounds_advanced));
            row["finished"] = json!(measured.last.as_ref().map(|result| result.finished));
            failed |= measured.errors != 0;
            println!("{row}");
        }
    }
    if failed {
        return Err("存在失败工作负载，请先检查 JSONL 错误再比较耗时".into());
    }
    Ok(())
}

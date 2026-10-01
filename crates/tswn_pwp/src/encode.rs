use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use tswn_core::encoder::batch::{EncodedBatch, TENSOR_SPECS, tensor_shape};
use tswn_core::encoder::capacity::EncoderProfile;
use tswn_core::encoder::encode::FeatureEncoder;
use tswn_core::encoder::manifest::EncoderManifest;

use crate::{DatasetConfig, SampleRow, storage};

#[derive(Debug, Clone, clap::Args)]
pub struct EncodeArgs {
    /// 已生成的 Parquet 数据集目录。
    #[arg(long)]
    pub dataset: PathBuf,
    /// 由 calibrate/外部组装生成的 EncoderManifest JSON。
    #[arg(long)]
    pub manifest: PathBuf,
    /// 输出编码包目录。
    #[arg(long)]
    pub out: PathBuf,
    /// 只编码指定 split。
    #[arg(long, default_value = "train")]
    pub split: String,
    /// 每批样本数；尾批使用实际 B。
    #[arg(long, default_value_t = 8)]
    pub batch_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchManifest {
    format: String,
    batch_index: usize,
    batch_size: usize,
    encoder_manifest_sha256: String,
    tensors: Vec<TensorFile>,
    rows_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TensorFile {
    name: String,
    dtype: String,
    shape: Vec<usize>,
    byte_length: usize,
    file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportManifest {
    format: String,
    state_schema_version: u32,
    split: String,
    batch_size: usize,
    encoder_manifest_sha256: String,
    batches: usize,
    samples: usize,
}

pub fn run(args: &EncodeArgs) -> Result<()> {
    ensure!(args.batch_size > 0, "batch-size 必须大于 0");
    let manifest: EncoderManifest = storage::read_json(&args.manifest).context("读取 encoder manifest")?;
    manifest.validate().map_err(|error| anyhow::anyhow!(error.to_string()))?;
    ensure!(
        manifest.contract_digest.as_deref() == Some(manifest.digest().as_str()),
        "encoder manifest contract_digest 不匹配"
    );
    let profile = profile_from_manifest(&manifest.profile);
    let encoder = FeatureEncoder::new(manifest.clone()).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let config: DatasetConfig = storage::read_json(&args.dataset.join("manifest.json")).context("读取数据集 manifest")?;
    ensure!(
        config.state_schema_version == manifest.state_schema_version,
        "state schema 与 encoder manifest 不一致"
    );
    fs::create_dir_all(&args.out)?;
    let mut rows = Vec::new();
    let total = config.cases.len() * config.games_per_matchup;
    let shard_count = total.div_ceil(config.battles_per_shard);
    for index in 0..shard_count {
        let path = args.dataset.join(format!("shard-{index:06}/samples.parquet"));
        storage::read_rows::<SampleRow>(&path, |row| {
            if row.split == args.split {
                rows.push(row);
            }
            Ok(())
        })
        .with_context(|| format!("读取分片 {index}"))?;
    }
    let mut batches = 0usize;
    for (batch_index, chunk) in rows.chunks(args.batch_size).enumerate() {
        let mut encoded = EncodedBatch::new(&profile, chunk.len());
        let mut row_hasher = Sha256::new();
        for (slot, row) in chunk.iter().enumerate() {
            row_hasher.update(row.battle_id.to_le_bytes());
            row_hasher.update(row.rounds_advanced.to_le_bytes());
            encoder
                .encode_into(&row.state, slot, &mut encoded)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        }
        let dir = args.out.join(format!("batch-{batch_index:06}"));
        fs::create_dir_all(&dir)?;
        let mut tensors = Vec::with_capacity(TENSOR_SPECS.len());
        for spec in TENSOR_SPECS {
            let bytes = encoded.tensor_bytes(spec.name).map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let file = format!("{}.bin", spec.name);
            fs::write(dir.join(&file), &bytes)?;
            let mut shape = tensor_shape(encoded.dims(), spec.name).expect("注册张量必须有 shape");
            shape.insert(0, chunk.len());
            tensors.push(TensorFile {
                name: spec.name.to_owned(),
                dtype: format!("{:?}", spec.dtype).to_lowercase(),
                shape,
                byte_length: bytes.len(),
                file,
            });
        }
        storage::write_json(
            &dir.join("batch-manifest.json"),
            &BatchManifest {
                format: "tswn-core/encoded-batch".to_owned(),
                batch_index,
                batch_size: chunk.len(),
                encoder_manifest_sha256: manifest.digest(),
                tensors,
                rows_sha256: crate::random::hex(&row_hasher.finalize()),
            },
        )?;
        batches += 1;
    }
    let encoder_digest = manifest.digest();
    storage::write_json(
        &args.out.join("manifest.json"),
        &ExportManifest {
            format: "tswn-core/encoded-dataset".to_owned(),
            state_schema_version: config.state_schema_version,
            split: args.split.clone(),
            batch_size: args.batch_size,
            encoder_manifest_sha256: encoder_digest,
            batches,
            samples: rows.len(),
        },
    )?;
    println!("编码完成：split={} 样本 {}，批次 {}", args.split, rows.len(), batches);
    Ok(())
}

fn profile_from_manifest(profile: &tswn_core::encoder::manifest::ProfileSpec) -> EncoderProfile {
    EncoderProfile {
        name: "encoded-manifest",
        e_max: profile.e_max,
        t_max: profile.t_max,
        r_max: profile.r_max,
        h_max: profile.h_max,
        l_max: profile.l_max,
        s_max: profile.s_max,
        q_max: profile.q_max,
        v_max: profile.v_max,
        x_max: profile.x_max,
    }
}

//! 将核心 encoder 的结果、监督标签与源行关联写入独立通道。
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tswn_core::{
    cli_api::battle::BattleModelFrame,
    encoder::{
        batch::{EncodedBatch, TENSOR_SPECS, tensor_shape},
        encode::FeatureEncoder,
        manifest::EncoderManifest,
    },
};

use crate::{DatasetConfig, SampleRow, storage};

#[derive(Debug, Clone, clap::Args)]
pub struct EncodeArgs {
    /// 已生成的 Parquet 数据集目录。
    #[arg(long)]
    pub dataset: PathBuf,
    /// build-manifest 生成的完整 EncoderManifest JSON。
    #[arg(long)]
    pub manifest: PathBuf,
    /// 输出编码包目录。
    #[arg(long)]
    pub out: PathBuf,
    /// 只编码指定 split。
    #[arg(long, default_value = "train", value_parser = ["train", "validation", "test"])]
    pub split: String,
    /// 每批样本数；尾批使用实际 B。
    #[arg(long, default_value_t = 8)]
    pub batch_size: usize,
    /// 诊断导出保留空标签；默认只导出有标签样本。
    #[arg(long)]
    pub keep_unlabeled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchManifest {
    format: String,
    format_version: u32,
    batch_index: usize,
    batch_size: usize,
    encoder_manifest_sha256: String,
    features: Vec<TensorFile>,
    exact: Vec<TensorFile>,
    labels: Vec<TensorFile>,
    audit: FileEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TensorFile {
    name: String,
    dtype: String,
    shape: Vec<usize>,
    #[serde(flatten)]
    payload: FileEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEntry {
    byte_length: usize,
    file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportManifest {
    format: String,
    format_version: u32,
    state_schema_version: u32,
    split: String,
    keep_unlabeled: bool,
    batch_size: usize,
    encoder_manifest_sha256: String,
    batches: usize,
    samples: usize,
}

/// 数组下标就是批内槽位；审计列不能由 features 清单枚举到。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct SourceRow {
    source_shard: usize,
    source_row: usize,
    battle_id: u64,
    matchup_id: String,
    frame: Option<BattleModelFrame>,
    rounds_advanced: usize,
    progress: f64,
}

struct SelectedRow {
    row: SampleRow,
    source: SourceRow,
}

pub fn run(args: &EncodeArgs) -> Result<()> {
    ensure!(args.batch_size > 0, "batch-size 必须大于 0");
    let manifest: EncoderManifest = storage::read_json(&args.manifest).context("读取 encoder manifest")?;
    manifest.validate().map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let digest = manifest.digest();
    ensure!(
        manifest.contract_digest.as_deref() == Some(digest.as_str()),
        "encoder manifest contract_digest 不匹配"
    );
    let encoder = FeatureEncoder::new(manifest.clone()).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let config: DatasetConfig = storage::read_json(&args.dataset.join("manifest.json")).context("读取数据集 manifest")?;
    ensure!(
        config.state_schema_version == manifest.state_schema_version,
        "state schema 与 encoder manifest 不一致"
    );
    ensure!(config.battles_per_shard > 0, "battles-per-shard 必须大于 0");
    let total = config.cases.len().checked_mul(config.games_per_matchup).context("对局数溢出")?;
    let mut rows = Vec::new();
    for index in 0..total.div_ceil(config.battles_per_shard) {
        let path = args.dataset.join(format!("shard-{index:06}/samples.parquet"));
        let mut row_index = 0;
        storage::read_rows::<SampleRow>(&path, |row| {
            let source_row = row_index;
            row_index += 1;
            if row.split == args.split && (args.keep_unlabeled || row.winner_team_index.is_some()) {
                rows.push(SelectedRow {
                    source: SourceRow {
                        source_shard: index,
                        source_row,
                        battle_id: row.battle_id,
                        matchup_id: row.matchup_id.clone(),
                        frame: row.frame,
                        rounds_advanced: row.rounds_advanced,
                        progress: row.progress,
                    },
                    row,
                });
            }
            Ok(())
        })
        .with_context(|| format!("读取分片 {index}"))?;
    }
    ensure!(!rows.is_empty(), "没有符合 split 和标签策略的样本");
    fs::create_dir_all(&args.out)?;
    storage::write_json(&args.out.join("encoder-manifest.json"), &manifest)?;
    let mut batches = 0;
    for (index, chunk) in rows.chunks(args.batch_size).enumerate() {
        write_batch(&args.out, index, chunk, &encoder, &digest)?;
        batches += 1;
    }
    storage::write_json(
        &args.out.join("manifest.json"),
        &ExportManifest {
            format: "tswn-core/encoded-dataset".to_owned(),
            format_version: 2,
            state_schema_version: config.state_schema_version,
            split: args.split.clone(),
            keep_unlabeled: args.keep_unlabeled,
            batch_size: args.batch_size,
            encoder_manifest_sha256: digest,
            batches,
            samples: rows.len(),
        },
    )?;
    println!("编码完成：split={} 样本 {}，批次 {}", args.split, rows.len(), batches);
    Ok(())
}

fn write_batch(out: &Path, index: usize, chunk: &[SelectedRow], encoder: &FeatureEncoder, digest: &str) -> Result<()> {
    let mut encoded = EncodedBatch::new(encoder.profile(), chunk.len());
    let mut winners = Vec::with_capacity(chunk.len() * 4);
    let mut label_mask = Vec::with_capacity(chunk.len());
    let mut sources = Vec::with_capacity(chunk.len());
    for (slot, selected) in chunk.iter().enumerate() {
        let row = &selected.row;
        let winner = match row.winner_team_index {
            Some(winner) => {
                ensure!(winner < row.state.input_teams.len(), "胜者输入队伍编号越界");
                i32::try_from(winner).context("胜者输入队伍编号超过 i32")?
            }
            None => -1,
        };
        winners.extend(winner.to_le_bytes());
        label_mask.push(u8::from(winner >= 0));
        encoder
            .encode_into(&row.state, slot, &mut encoded)
            .map_err(|error| anyhow::anyhow!(error.to_string()))
            .with_context(|| {
                format!(
                    "分片 {} 行 {}，battle_id={}",
                    selected.source.source_shard, selected.source.source_row, row.battle_id
                )
            })?;
        sources.push(&selected.source);
    }
    let dir = out.join(format!("batch-{index:06}"));
    fs::create_dir_all(&dir)?;
    let mut features = Vec::new();
    let mut exact = Vec::new();
    for spec in TENSOR_SPECS {
        let bytes = encoded.tensor_bytes(spec.name).map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let mut shape = tensor_shape(encoded.dims(), spec.name).expect("注册张量必须有 shape");
        shape.insert(0, chunk.len());
        let tensor = write_tensor(&dir, spec.name, spec.dtype.suffix(), shape, &bytes)?;
        // 原始 bit 与精确注册序保留在关系/校验旁路，不自动交给数值模型。
        if matches!(spec.name, "extra_bits" | "order_key") {
            exact.push(tensor);
        } else {
            features.push(tensor);
        }
    }
    let labels = vec![
        write_tensor(&dir, "winner_team_index", "i32", vec![chunk.len()], &winners)?,
        write_tensor(&dir, "label_mask", "u8", vec![chunk.len()], &label_mask)?,
    ];
    let audit_bytes = serde_json::to_vec(&sources)?;
    fs::write(dir.join("rows.json"), &audit_bytes)?;
    storage::write_json(
        &dir.join("batch-manifest.json"),
        &BatchManifest {
            format: "tswn-core/encoded-batch".to_owned(),
            format_version: 2,
            batch_index: index,
            batch_size: chunk.len(),
            encoder_manifest_sha256: digest.to_owned(),
            features,
            exact,
            labels,
            audit: FileEntry {
                byte_length: audit_bytes.len(),
                file: "rows.json".to_owned(),
            },
        },
    )?;
    Ok(())
}

fn write_tensor(dir: &Path, name: &str, dtype: &str, shape: Vec<usize>, bytes: &[u8]) -> Result<TensorFile> {
    let file = format!("{name}.bin");
    fs::write(dir.join(&file), bytes)?;
    Ok(TensorFile {
        name: name.to_owned(),
        dtype: dtype.to_owned(),
        shape,
        payload: FileEntry {
            byte_length: bytes.len(),
            file,
        },
    })
}

#[cfg(test)]
mod tests;

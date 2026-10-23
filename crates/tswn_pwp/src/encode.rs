//! 将核心 encoder 的结果、监督标签与源行关联写入独立通道。
//!
//! 先校验编码契约和源分片身份，再按 split/标签策略流式填充固定批；尾批使用实际数量。
//! 源目录共享锁在整个导出期间持有，特征、精确旁路、标签和 audit 使用不同清单。
//! 输出先写同级暂存包并逐文件回读哈希，全部成功才发布目录；失败保留现场且不覆盖旧包。
mod output;
mod source;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tswn_core::{
    cli_api::battle::BattleModelFrame,
    encoder::{
        batch::{CapacityDims, EncodedBatch, TENSOR_SPECS, tensor_shape},
        encode::FeatureEncoder,
        manifest::EncoderManifest,
    },
};

use crate::{SampleRow, storage};
use source::SourceDataset;

#[derive(Debug, Clone, clap::Args)]
pub struct EncodeArgs {
    /// 已生成的 Parquet 数据集目录。
    #[arg(long)]
    pub dataset: PathBuf,
    /// build-manifest 生成的完整 EncoderManifest JSON。
    #[arg(long)]
    pub manifest: PathBuf,
    /// 新的编码包目录，必须不存在；失败临时包保留在同级目录。
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
    sha256: String,
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
    encoder_manifest: FileEntry,
    dataset_manifest_sha256: String,
    dataset_sha256: String,
    tensor_bytes: u64,
    batch_manifest_sha256: Vec<String>,
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
    output::ensure_absent(&args.out)?;
    let manifest: EncoderManifest = storage::read_json(&args.manifest).context("读取 encoder manifest")?;
    manifest.validate().map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let digest = manifest.digest();
    ensure!(
        manifest.contract_digest.as_deref() == Some(digest.as_str()),
        "encoder manifest contract_digest 不匹配"
    );
    let encoder = FeatureEncoder::new(manifest.clone()).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let source = SourceDataset::open(args)?;
    ensure!(
        source.config.state_schema_version == manifest.state_schema_version,
        "state schema 与 encoder manifest 不一致"
    );
    ensure!(source.selected_samples > 0, "没有符合 split 和标签策略的样本");
    let tensor_bytes = tensor_budget(&encoder, source.selected_samples)?;
    let output = output::OutputDirectory::create(&args.out, tensor_bytes)?;
    write_export(args, &source, &encoder, &digest, tensor_bytes, &output)
        .with_context(|| format!("编码失败；临时包保留在 {}", output.staging.display()))?;
    println!(
        "编码完成：split={} 样本 {}，批次 {}",
        args.split,
        source.selected_samples,
        source.selected_samples.div_ceil(args.batch_size)
    );
    Ok(())
}

fn write_export(
    args: &EncodeArgs,
    source: &SourceDataset,
    encoder: &FeatureEncoder,
    digest: &str,
    tensor_bytes: u64,
    output: &output::OutputDirectory,
) -> Result<()> {
    let encoder_manifest = output::write_payload(
        &output.staging,
        "encoder-manifest.json",
        &serde_json::to_vec_pretty(encoder.manifest())?,
    )?;
    let mut batch_manifest_sha256 = Vec::new();
    let samples = stream_batches(args, source, |chunk| {
        let sha256 = write_batch(&output.staging, batch_manifest_sha256.len(), chunk, encoder, digest)?;
        batch_manifest_sha256.push(sha256);
        Ok(())
    })?;
    storage::write_json(
        &output.staging.join("manifest.json"),
        &ExportManifest {
            format: "tswn-core/encoded-dataset".to_owned(),
            format_version: 2,
            state_schema_version: source.config.state_schema_version,
            split: args.split.clone(),
            keep_unlabeled: args.keep_unlabeled,
            batch_size: args.batch_size,
            encoder_manifest_sha256: digest.to_owned(),
            encoder_manifest,
            dataset_manifest_sha256: source.manifest_sha256.clone(),
            dataset_sha256: source.dataset_sha256.clone(),
            tensor_bytes,
            batches: batch_manifest_sha256.len(),
            batch_manifest_sha256,
            samples,
        },
    )?;
    output.commit()
}

/// 当前批跨源分片连续填充；回调成功后立即释放这一批状态，失败则停止读取。
fn stream_batches(
    args: &EncodeArgs,
    source: &SourceDataset,
    mut visit: impl FnMut(&[SelectedRow]) -> Result<()>,
) -> Result<usize> {
    let mut pending = Vec::with_capacity(args.batch_size.min(source.selected_samples));
    let mut samples = 0usize;
    for (index, receipt) in source.receipts.iter().enumerate() {
        let path = args.dataset.join(format!("shard-{index:06}/samples.parquet"));
        let mut row_index = 0usize;
        storage::read_rows::<SampleRow>(&path, |row| {
            let source_row = row_index;
            row_index += 1;
            if row.split == args.split && (args.keep_unlabeled || row.winner_team_index.is_some()) {
                pending.push(SelectedRow {
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
                samples = samples.checked_add(1).context("样本数溢出")?;
                if pending.len() == args.batch_size {
                    visit(&pending)?;
                    pending.clear();
                }
            }
            Ok(())
        })
        .with_context(|| format!("读取分片 {index}"))?;
        ensure!(row_index == receipt.samples, "源分片 {index} 的样本数在编码期间发生变化");
    }
    if !pending.is_empty() {
        visit(&pending)?;
    }
    ensure!(samples == source.selected_samples, "选中样本数在编码期间发生变化");
    Ok(samples)
}

fn tensor_budget(encoder: &FeatureEncoder, samples: usize) -> Result<u64> {
    let dims = encoder.profile().dims();
    let mut per_sample = 5usize; // i32 标签与 u8 label_mask。
    for tensor in TENSOR_SPECS {
        let shape = tensor_shape(&dims, tensor.name).context("张量 shape 未登记")?;
        let elements = shape
            .iter()
            .try_fold(1usize, |product, axis| product.checked_mul(*axis))
            .context("张量 shape 溢出")?;
        let bytes = elements.checked_mul(tensor.dtype.size()).context("张量字节数溢出")?;
        per_sample = per_sample.checked_add(bytes).context("每样本字节数溢出")?;
    }
    u64::try_from(per_sample)?
        .checked_mul(u64::try_from(samples)?)
        .context("编码包字节预算溢出")
}

fn write_batch(out: &Path, index: usize, chunk: &[SelectedRow], encoder: &FeatureEncoder, digest: &str) -> Result<String> {
    let mut encoded = EncodedBatch::new(encoder.profile(), chunk.len());
    let mut winners = Vec::with_capacity(chunk.len() * 4);
    let mut label_mask = Vec::with_capacity(chunk.len());
    let mut sources = Vec::with_capacity(chunk.len());
    for (slot, selected) in chunk.iter().enumerate() {
        let row = &selected.row;
        let winner = row
            .winner_team_index
            .map(|winner| -> Result<i32> {
                ensure!(winner < row.state.input_teams.len(), "胜者输入队伍编号越界");
                i32::try_from(winner).context("胜者输入队伍编号超过 i32")
            })
            .transpose()
            .with_context(|| {
                format!(
                    "分片 {} 行 {}，battle_id={}",
                    selected.source.source_shard, selected.source.source_row, row.battle_id
                )
            })?
            .unwrap_or(-1);
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
    fs::create_dir(&dir)?;
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
    let audit = output::write_payload(&dir, "rows.json", &audit_bytes)?;
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
            audit,
        },
    )?;
    storage::file_hash(&dir.join("batch-manifest.json"))
}

fn write_tensor(dir: &Path, name: &str, dtype: &str, shape: Vec<usize>, bytes: &[u8]) -> Result<TensorFile> {
    let file = format!("{name}.bin");
    let payload = output::write_payload(dir, &file, bytes)?;
    Ok(TensorFile {
        name: name.to_owned(),
        dtype: dtype.to_owned(),
        shape,
        payload,
    })
}

#[cfg(test)]
mod tests;

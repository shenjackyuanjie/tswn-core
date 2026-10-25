//! 固定源数据身份，先投影扫描标签计数；此阶段不反序列化机制状态。
//!
//! SourceDataset 拥有生成器锁文件的共享锁，存活期间阻止合作生成器续写。
//! manifest 与完成回执组成数据集身份，分片文件哈希和标签行数在编码前核对。
//! 这不是完整 state 语义检查，逐行编码和独立 validate 仍负责相应约束。
use anyhow::{Context, Result, ensure};
use parquet::arrow::{ProjectionMask, arrow_reader::ParquetRecordBatchReaderBuilder};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs::File, path::Path};

use super::EncodeArgs;
use crate::{
    DatasetConfig,
    storage::{self, ShardReceipt},
    validate,
};

pub struct SourceDataset {
    pub config: DatasetConfig,
    pub receipts: Vec<ShardReceipt>,
    pub manifest_sha256: String,
    pub dataset_sha256: String,
    pub selected_samples: usize,
    // 文件存活期间持有共享锁；不创建或修改源数据目录中的文件。
    _lock: File,
}

impl SourceDataset {
    pub fn open(args: &EncodeArgs) -> Result<Self> {
        let lock = File::open(args.dataset.join(".generator.lock")).context("打开源数据集锁；请使用 generate 已完成的数据集")?;
        fs2::FileExt::try_lock_shared(&lock).context("源数据集正在生成或续跑，暂不能编码")?;
        let manifest_path = args.dataset.join("manifest.json");
        let config: DatasetConfig = storage::read_json(&manifest_path).context("读取数据集 manifest")?;
        let total = validate::check_config(&config)?;
        let manifest_sha256 = storage::file_hash(&manifest_path)?;
        let mut receipts = Vec::new();
        let mut selected_samples = 0usize;
        for index in 0..total.div_ceil(config.battles_per_shard) {
            let dir = args.dataset.join(format!("shard-{index:06}"));
            let (first, end) = crate::generate::range(&config, index, total);
            let receipt = storage::check_receipt(&dir, first, end).with_context(|| format!("检查源分片 {index}"))?;
            let (seen, selected) =
                count_selected(&dir.join("samples.parquet"), args).with_context(|| format!("扫描源分片 {index} 的标签"))?;
            ensure!(seen == receipt.samples, "源分片 {index} 的样本数与完成记录不匹配");
            selected_samples = selected_samples.checked_add(selected).context("样本数溢出")?;
            receipts.push(receipt);
        }
        let identity = serde_json::to_vec(&(&manifest_sha256, &receipts))?;
        let dataset_sha256 = crate::random::hex(&Sha256::digest(&identity));
        Ok(Self {
            config,
            receipts,
            manifest_sha256,
            dataset_sha256,
            selected_samples,
            _lock: lock,
        })
    }
}

#[derive(Deserialize)]
struct Selection {
    split: String,
    winner_team_index: Option<usize>,
}

fn count_selected(path: &Path, args: &EncodeArgs) -> Result<(usize, usize)> {
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?;
    let indices = [
        builder.schema().index_of("split")?,
        builder.schema().index_of("winner_team_index")?,
    ];
    let projection = ProjectionMask::roots(builder.parquet_schema(), indices);
    let reader = builder.with_projection(projection).with_batch_size(1024).build()?;
    let mut seen = 0usize;
    let mut selected = 0usize;
    for batch in reader {
        let rows: Vec<Selection> = serde_arrow::from_record_batch(&batch?)?;
        seen = seen.checked_add(rows.len()).context("分片样本数溢出")?;
        for row in rows {
            if row.split == args.split && (args.keep_unlabeled || row.winner_team_index.is_some()) {
                selected = selected.checked_add(1).context("选中样本数溢出")?;
            }
        }
    }
    Ok((seen, selected))
}

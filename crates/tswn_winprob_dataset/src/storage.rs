use anyhow::{Context, Result, ensure};
use arrow_schema::{DataType, FieldRef, Schema};
use parquet::{
    arrow::{ArrowWriter, arrow_reader::ParquetRecordBatchReaderBuilder},
    basic::{Compression, ZstdLevel},
    file::properties::WriterProperties,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_arrow::schema::{SchemaLike, TracingOptions};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    marker::PhantomData,
    path::Path,
    sync::Arc,
};

pub fn file_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("读取 {}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(crate::random::hex(&hash.finalize()))
}

pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file = File::create(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_reader(File::open(path)?).with_context(|| format!("解析 {}", path.display()))
}

pub fn fields<T: DeserializeOwned>() -> Result<Vec<FieldRef>> {
    let fields =
        Vec::<FieldRef>::from_type::<T>(TracingOptions::default().enums_without_data_as_strings(true).from_type_budget(10_000))?;
    Ok(fields.iter().map(portable_field).collect())
}

/// 嵌套逻辑字典跨行组时，PyArrow 可能无法拼接不同字典的子数组。
/// 将无载荷枚举写成普通字符串；Parquet 自身的物理字典压缩仍可使用。
fn portable_field(field: &FieldRef) -> FieldRef {
    let data_type = match field.data_type() {
        DataType::Dictionary(_, value) => value.as_ref().clone(),
        DataType::Struct(fields) => DataType::Struct(fields.iter().map(portable_field).collect()),
        DataType::List(child) => DataType::List(portable_field(child)),
        DataType::LargeList(child) => DataType::LargeList(portable_field(child)),
        DataType::FixedSizeList(child, size) => DataType::FixedSizeList(portable_field(child), *size),
        other => other.clone(),
    };
    Arc::new(field.as_ref().clone().with_data_type(data_type))
}

pub struct TableWriter<T> {
    writer: ArrowWriter<File>,
    fields: Vec<FieldRef>,
    marker: PhantomData<T>,
    pub rows: usize,
}
impl<T: Serialize + DeserializeOwned> TableWriter<T> {
    pub fn create(path: &Path) -> Result<Self> {
        let fields = fields::<T>()?;
        let properties = WriterProperties::builder()
            .set_compression(Compression::ZSTD(ZstdLevel::try_new(3)?))
            .set_max_row_group_row_count(Some(10_000))
            .set_max_row_group_bytes(Some(16 * 1024 * 1024))
            .build();
        let writer = ArrowWriter::try_new(File::create(path)?, Arc::new(Schema::new(fields.clone())), Some(properties))?;
        Ok(Self {
            writer,
            fields,
            marker: PhantomData,
            rows: 0,
        })
    }
    pub fn append(&mut self, rows: &[T]) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let batch = serde_arrow::to_record_batch(&self.fields, &rows)?;
        self.writer.write(&batch)?;
        self.rows += rows.len();
        if self.writer.memory_size() >= 16 * 1024 * 1024 {
            self.writer.flush()?;
        }
        Ok(())
    }
    pub fn finish(self) -> Result<usize> {
        let count = self.rows;
        let file = self.writer.into_inner()?;
        file.sync_all()?;
        Ok(count)
    }
}

pub fn read_rows<T: DeserializeOwned>(path: &Path, mut visit: impl FnMut(T) -> Result<()>) -> Result<()> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?
        .with_batch_size(128)
        .build()?;
    for batch in reader {
        let batch = batch?;
        let rows: Vec<T> = serde_arrow::from_record_batch(&batch)?;
        for row in rows {
            visit(row)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardReceipt {
    pub first_battle: usize,
    pub end_battle: usize,
    pub battles: usize,
    pub samples: usize,
    pub battles_sha256: String,
    pub samples_sha256: String,
}

pub fn check_receipt(dir: &Path, first: usize, end: usize) -> Result<ShardReceipt> {
    let receipt: ShardReceipt = read_json(&dir.join("complete.json"))?;
    ensure!(
        receipt.first_battle == first && receipt.end_battle == end && receipt.battles == end - first,
        "分片对局范围不匹配：{}",
        dir.display()
    );
    ensure!(
        file_hash(&dir.join("battles.parquet"))? == receipt.battles_sha256,
        "对局文件校验失败：{}",
        dir.display()
    );
    ensure!(
        file_hash(&dir.join("samples.parquet"))? == receipt.samples_sha256,
        "样本文件校验失败：{}",
        dir.display()
    );
    Ok(receipt)
}

/// 只删除当前输出目录下由本程序管理的临时分片，拒绝链接跳转。
pub fn remove_incomplete(out: &Path, name: &str) -> Result<()> {
    ensure!(
        name.starts_with(".shard-") && name.ends_with(".tmp") && !name.contains(['/', '\\']),
        "无效临时分片名"
    );
    let root = fs::canonicalize(out)?;
    let path = root.join(name);
    if !path.try_exists()? {
        return Ok(());
    }
    ensure!(!fs::symlink_metadata(&path)?.file_type().is_symlink(), "临时分片不能是符号链接");
    ensure!(
        fs::canonicalize(&path)?.parent() == Some(root.as_path()),
        "临时分片越过输出目录"
    );
    fs::remove_dir_all(path)?;
    Ok(())
}

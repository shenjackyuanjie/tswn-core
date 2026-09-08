//! 生成与校验的规模基准采集。
//!
//! 只在本进程内运行 `generate`/`validate` 并采样自身资源占用，再读取产物的 Parquet 元数据；
//! 不改变数据集格式，也不重新实现任何抽样或校验逻辑。

use crate::{GenerateArgs, storage};
use anyhow::{Context, Result};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Debug, Clone, clap::Args)]
pub struct BenchArgs {
    #[command(flatten)]
    pub generate: GenerateArgs,
    /// 资源采样间隔（秒）。
    #[arg(long, default_value_t = 0.2)]
    pub interval: f64,
    /// 报告里保留的列路径条数。
    #[arg(long, default_value_t = 20)]
    pub top: usize,
    /// 只分析已有产物，不重新生成。
    #[arg(long)]
    pub skip_generate: bool,
    #[arg(long, default_value = "run")]
    pub label: String,
    /// 额外把完整结果写入该 JSON 文件。
    #[arg(long)]
    pub json_out: Option<PathBuf>,
}

#[derive(Debug, Default, Clone)]
struct Sample {
    peak_rss_bytes: u64,
    peak_threads: usize,
    cpu_ms: u64,
    polls: usize,
}

/// 在后台线程里采样本进程的 RSS、线程数与 CPU 时间。
struct Sampler {
    stop: Arc<AtomicBool>,
    handle: std::thread::JoinHandle<Sample>,
    started: Instant,
}

impl Sampler {
    fn start(interval: Duration) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::spawn(move || {
            let pid = Pid::from_u32(std::process::id());
            let mut system = System::new();
            let mut sample = Sample::default();
            while !flag.load(Ordering::Relaxed) {
                system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                if let Some(process) = system.process(pid) {
                    sample.peak_rss_bytes = sample.peak_rss_bytes.max(process.memory());
                    sample.peak_threads = sample.peak_threads.max(process.tasks().map_or(0, |tasks| tasks.len()));
                    sample.cpu_ms = sample.cpu_ms.max(process.accumulated_cpu_time());
                    sample.polls += 1;
                }
                std::thread::sleep(interval);
            }
            sample
        });
        Self {
            stop,
            handle,
            started: Instant::now(),
        }
    }
    fn stop(self) -> (Sample, f64) {
        self.stop.store(true, Ordering::Relaxed);
        let sample = self.handle.join().unwrap_or_default();
        (sample, self.started.elapsed().as_secs_f64())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetrics {
    pub wall_seconds: f64,
    pub peak_rss_bytes: u64,
    pub peak_threads: usize,
    pub cpu_seconds: f64,
    /// 相对单核的占用比例；多线程时可大于 1。
    pub cpu_single_core: f64,
    pub polls: usize,
}

impl RunMetrics {
    fn new(sample: Sample, wall: f64) -> Self {
        let cpu_seconds = sample.cpu_ms as f64 / 1000.0;
        Self {
            wall_seconds: wall,
            peak_rss_bytes: sample.peak_rss_bytes,
            peak_threads: sample.peak_threads,
            cpu_seconds,
            cpu_single_core: if wall > 0.0 { cpu_seconds / wall } else { 0.0 },
            polls: sample.polls,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TableStats {
    pub file_bytes: u64,
    pub rows: usize,
    pub row_groups: usize,
    pub column_compressed_bytes: u64,
    pub column_uncompressed_bytes: u64,
    pub compression_ratio: Option<f64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DatasetStats {
    pub shards: usize,
    pub battles: usize,
    pub samples: usize,
    pub file_bytes: u64,
    pub bytes_per_battle: f64,
    pub bytes_per_sample: f64,
    pub tables: BTreeMap<String, TableStats>,
    /// 按列路径前 1~3 级聚合的压缩字节。
    pub groups: BTreeMap<String, Vec<ColumnShare>>,
    /// 压缩字节最高的叶子列。
    pub top_columns: Vec<ColumnShare>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnShare {
    pub path: String,
    pub compressed_bytes: u64,
    pub uncompressed_bytes: u64,
    pub share_of_samples_columns: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    pub label: String,
    pub generate: Option<RunMetrics>,
    pub validate: RunMetrics,
    pub dataset: DatasetStats,
}

/// 单个叶子列的（压缩字节，未压缩字节）。
type ColumnBytes = (u64, u64);
/// 列路径到字节数的映射。
type ColumnMap = BTreeMap<String, ColumnBytes>;

/// 读取单个 Parquet 文件的元数据，按叶子列聚合压缩字节。
fn table_stats(path: &Path) -> Result<(TableStats, ColumnMap)> {
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?;
    let metadata = builder.metadata();
    let mut stats = TableStats {
        file_bytes: std::fs::metadata(path)?.len(),
        rows: metadata.file_metadata().num_rows() as usize,
        row_groups: metadata.num_row_groups(),
        ..TableStats::default()
    };
    let mut columns: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for group in 0..metadata.num_row_groups() {
        let row_group = metadata.row_group(group);
        for index in 0..row_group.num_columns() {
            let column = row_group.column(index);
            let compressed = column.compressed_size() as u64;
            let uncompressed = column.uncompressed_size() as u64;
            stats.column_compressed_bytes += compressed;
            stats.column_uncompressed_bytes += uncompressed;
            let entry = columns.entry(column.column_path().string().to_owned()).or_default();
            entry.0 += compressed;
            entry.1 += uncompressed;
        }
    }
    stats.compression_ratio = (stats.column_compressed_bytes > 0)
        .then(|| stats.column_uncompressed_bytes as f64 / stats.column_compressed_bytes as f64);
    Ok((stats, columns))
}

fn analyze_dataset(out: &Path, top: usize) -> Result<DatasetStats> {
    let shards: Vec<PathBuf> = std::fs::read_dir(out)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("shard-")))
        .collect();
    anyhow::ensure!(!shards.is_empty(), "{} 下没有已完成分片", out.display());
    let mut stats = DatasetStats {
        shards: shards.len(),
        ..DatasetStats::default()
    };
    let mut merged: ColumnMap = BTreeMap::new();
    for shard in &shards {
        for name in ["battles.parquet", "samples.parquet"] {
            let (table, columns) = table_stats(&shard.join(name))?;
            stats.file_bytes += table.file_bytes;
            let entry = stats.tables.entry(name.into()).or_default();
            entry.file_bytes += table.file_bytes;
            entry.rows += table.rows;
            entry.row_groups += table.row_groups;
            entry.column_compressed_bytes += table.column_compressed_bytes;
            entry.column_uncompressed_bytes += table.column_uncompressed_bytes;
            for (path, value) in columns {
                let slot = merged.entry(path).or_default();
                slot.0 += value.0;
                slot.1 += value.1;
            }
        }
    }
    for table in stats.tables.values_mut() {
        table.compression_ratio = (table.column_compressed_bytes > 0)
            .then(|| table.column_uncompressed_bytes as f64 / table.column_compressed_bytes as f64);
    }
    stats.battles = stats.tables.get("battles.parquet").map_or(0, |table| table.rows);
    stats.samples = stats.tables.get("samples.parquet").map_or(0, |table| table.rows);
    let sample_columns = stats.tables.get("samples.parquet").map_or(0, |table| table.column_compressed_bytes);
    stats.bytes_per_battle = stats.file_bytes as f64 / stats.battles.max(1) as f64;
    stats.bytes_per_sample = stats.file_bytes as f64 / stats.samples.max(1) as f64;
    for depth in 1..=3 {
        let mut buckets: BTreeMap<String, u64> = BTreeMap::new();
        for (path, (compressed, _)) in &merged {
            let key = path.split('.').take(depth).collect::<Vec<_>>().join(".");
            *buckets.entry(key).or_default() += compressed;
        }
        let mut items: Vec<ColumnShare> = buckets
            .into_iter()
            .map(|(path, compressed_bytes)| ColumnShare {
                path,
                compressed_bytes,
                uncompressed_bytes: 0,
                share_of_samples_columns: share(compressed_bytes, sample_columns),
            })
            .collect();
        items.sort_by_key(|item| std::cmp::Reverse(item.compressed_bytes));
        items.truncate(top);
        stats.groups.insert(depth.to_string(), items);
    }
    let mut leaves: Vec<ColumnShare> = merged
        .into_iter()
        .map(|(path, (compressed_bytes, uncompressed_bytes))| ColumnShare {
            path,
            compressed_bytes,
            uncompressed_bytes,
            share_of_samples_columns: share(compressed_bytes, sample_columns),
        })
        .collect();
    leaves.sort_by_key(|item| std::cmp::Reverse(item.compressed_bytes));
    leaves.truncate(top);
    stats.top_columns = leaves;
    Ok(stats)
}

fn share(bytes: u64, total: u64) -> f64 { if total == 0 { 0.0 } else { bytes as f64 / total as f64 } }

pub fn run(args: &BenchArgs) -> Result<()> {
    let interval = Duration::from_secs_f64(args.interval.max(0.01));
    let generate = if args.skip_generate {
        None
    } else {
        let sampler = Sampler::start(interval);
        let result = crate::generate::generate(&args.generate);
        let (sample, wall) = sampler.stop();
        result?;
        Some(RunMetrics::new(sample, wall))
    };
    let sampler = Sampler::start(interval);
    let validation = crate::validate::validate_dataset(&args.generate.out);
    let (sample, wall) = sampler.stop();
    validation?;
    let validate = RunMetrics::new(sample, wall);
    let dataset = analyze_dataset(&args.generate.out, args.top).context("分析产物")?;
    let report = BenchReport {
        label: args.label.clone(),
        generate,
        validate,
        dataset,
    };
    if let Some(path) = &args.json_out {
        storage::write_json(path, &report)?;
    }
    print_report(&report, args.top);
    Ok(())
}

fn print_report(report: &BenchReport, top: usize) {
    let dataset = &report.dataset;
    println!();
    println!("## {}", report.label);
    println!();
    if let Some(generate) = &report.generate {
        println!(
            "- 生成墙钟：{:.2} s，{:.1} 对局/s，{:.1} 样本/s",
            generate.wall_seconds,
            dataset.battles as f64 / generate.wall_seconds,
            dataset.samples as f64 / generate.wall_seconds
        );
        println!(
            "- 生成峰值 RSS：{:.1} MiB，峰值线程：{}，CPU 时间：{:.1} s，CPU 利用率：{:.0}% 单核",
            generate.peak_rss_bytes as f64 / 2f64.powi(20),
            generate.peak_threads,
            generate.cpu_seconds,
            generate.cpu_single_core * 100.0
        );
    }
    println!(
        "- 校验墙钟：{:.2} s，{:.1} 对局/s，峰值 RSS：{:.1} MiB",
        report.validate.wall_seconds,
        dataset.battles as f64 / report.validate.wall_seconds,
        report.validate.peak_rss_bytes as f64 / 2f64.powi(20)
    );
    println!("- 对局 {}，样本 {}，分片 {}", dataset.battles, dataset.samples, dataset.shards);
    println!(
        "- 总文件：{:.1} MiB，每对局 {:.0} B，每样本 {:.0} B",
        dataset.file_bytes as f64 / 2f64.powi(20),
        dataset.bytes_per_battle,
        dataset.bytes_per_sample
    );
    for (name, table) in &dataset.tables {
        let ratio = table.compression_ratio.map_or_else(|| "-".to_owned(), |ratio| format!("{ratio:.2}x"));
        println!(
            "- `{name}`：{:.2} MiB，{} 行，{} 行组，列块压缩比 {ratio}",
            table.file_bytes as f64 / 2f64.powi(20),
            table.rows,
            table.row_groups
        );
    }
    for depth in ["1", "2", "3"] {
        let Some(items) = dataset.groups.get(depth) else {
            continue;
        };
        println!();
        println!("按路径前 {depth} 级聚合（samples 列块压缩字节）：");
        println!();
        println!("| 路径 | 压缩字节 | 占 samples 列块 |");
        println!("| --- | --- | --- |");
        for item in items.iter().take(top) {
            println!(
                "| `{}` | {:.1} KiB | {:.1}% |",
                item.path,
                item.compressed_bytes as f64 / 1024.0,
                item.share_of_samples_columns * 100.0
            );
        }
    }
    println!();
    println!("压缩字节最高的 {top} 个叶子列：");
    println!();
    println!("| 列路径 | 压缩字节 | 占 samples 列块 |");
    println!("| --- | --- | --- |");
    for item in &dataset.top_columns {
        println!(
            "| `{}` | {:.1} KiB | {:.1}% |",
            item.path,
            item.compressed_bytes as f64 / 1024.0,
            item.share_of_samples_columns * 100.0
        );
    }
}

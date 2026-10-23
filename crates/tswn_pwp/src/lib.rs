//! 胜率学习数据工具：确定性采样、Parquet 分片、校准和离线张量包。
//!
//! # 已实现的管线
//!
//! ```text
//! generate → validate → stats / calibrate → build-manifest → encode
//!    ↓          ↓                             ↓              ↓
//! Parquet    摘要/引用检查                 EncoderManifest   v2 训练包
//! ```
//!
//! [`run`] 分发 [`Cli`] / [`Command`]，binary 为 `tswn-pwp`。
//! `generate` 使用 core 的 BattleModelSession，按相同 seed 跑两遍：先确认边界与结果，
//! 再导出选中的状态。机制 state 保存当前机制输入，名字、seed、标签与审计独立组织。
//! 同一规范化阵容的 seed 与队伍排列变体使用同一 train/validation/test 切分组。
//!
//! # 对外数据
//!
//! - [`DatasetConfig`]：保存续跑校验所需的可复现配置。
//! - [`BattleRow`]：每局 seed、结果和样本计数。
//! - [`SampleRow`]：帧末机制 state、可空标签和来源审计。
//! - `encode`：调用 core 唯一的 FeatureEncoder，输出小端 C 连续张量、mask 与精确旁路。
//!
//! # 文件与失败策略
//!
//! 分片先写临时目录，回读校验后提交；输出目录有进程文件锁。
//! `--resume` 校验输入、配置和可执行文件摘要；发现已提交分片损坏时保留现场并报错。
//! Runtime 错误、unwind panic 或两遍不一致时尽力保存复现信息，等待各 worker 后返回失败；
//! 强制终止、abort 和掉电不会执行 Drop，不保证留下当前对局的复现文件。
//! `stats` / `validate` / `calibrate` 读取数据，`generate` / `bench` / `encode` 会写文件。
//!
//! 当前已完成数据与编码链路；模型训练、推理和网页在线接入可沿此继续建设。
//! 命令、目录格式、标签与 NumPy 读取器见
//! [PWP README](https://github.com/shenjackyuanjie/tswn-core/tree/main/crates/tswn_pwp)。
mod bench;
mod calibrate;
mod capacity;
mod encode;
mod generate;
mod input;
mod manifest;
mod random;
mod sampling;
mod stats;
mod storage;
mod validate;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tswn_core::cli_api::battle::{BattleModelFrame, BattleModelOutcome};
use tswn_core::runtime::model_state::BattleModelState;

#[derive(Debug, Parser)]
#[command(name = "tswn-pwp", about = "生成或校验战斗机制状态数据集")]
/// tswn-pwp 的 Clap 参数根，亦可构造后交给 [`run`]。
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
/// 数据生成、检查、统计、校准与编码子命令。
pub enum Command {
    Generate(GenerateArgs),
    Validate {
        #[arg(long)]
        out: PathBuf,
    },
    /// 汇总已完成数据集的分布统计，不修改数据。
    Stats(stats::StatsArgs),
    /// 采集生成与校验的规模基准；默认生成数据，跳过生成时只读已有产物。
    Bench(bench::BenchArgs),
    /// 从源数据拟合逐字段归一化常数，可另写报告，不修改源分片。
    Calibrate(calibrate::CalibrateArgs),
    /// 将有标签 train 校准报告组装并校验为完整编码契约。
    BuildManifest(manifest::BuildManifestArgs),
    /// 将 Parquet state 编码为按张量分片的 C 连续小端二进制。
    Encode(encode::EncodeArgs),
}

#[derive(Debug, Clone, Args)]
/// 生成配置：输入源、固定 seed、抽样、分片、并行度与输出目录。
///
/// 文件输入与名字池二选一，生成前还会检查正次数和有限 eval_rq。threads 只影响分片调度，
/// 不进入 DatasetConfig 的续跑身份；其他复现配置及输入/可执行文件摘要必须与旧 manifest 一致。
pub struct GenerateArgs {
    /// 固定对局文件，或递归读取其中 .txt 文件的目录。
    #[arg(
        long,
        conflicts_with = "names",
        required_unless_present = "names"
    )]
    pub input: Option<PathBuf>,
    /// 每行一个角色的名字池。
    #[arg(long, conflicts_with = "input", requires_all = ["team_sizes", "matchups"])]
    pub names: Option<PathBuf>,
    #[arg(long, value_delimiter = ',', requires = "names")]
    pub team_sizes: Vec<usize>,
    #[arg(long, requires = "names")]
    pub matchups: Option<usize>,
    #[arg(long)]
    pub games_per_matchup: usize,
    #[arg(long)]
    pub seed: String,
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long, default_value_t = 8)]
    pub samples_per_game: usize,
    #[arg(long, default_value_t = 1000)]
    pub battles_per_shard: usize,
    /// 0 使用可用 CPU 数，最多同时处理分片数量个任务。
    #[arg(long, default_value_t = 0)]
    pub threads: usize,
    #[arg(long, default_value_t = tswn_core::runtime::BINDING_COMPLETION_MAX_ROUNDS)]
    pub max_rounds: usize,
    #[arg(long, default_value_t = tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ)]
    pub eval_rq: f64,
    #[arg(long)]
    pub resume: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// 写入 manifest 的可复现配置与摘要，续跑时对整份值逐字段比较。
///
/// cases 保留输入来源、原始文本和规范队伍；input_sha256 绑定输入字节而非仅阵容集合。
/// 不包含 worker 数，因此可改变并行度续跑；可执行文件变化则不能与旧分片混合。
pub struct DatasetConfig {
    pub format_version: u32,
    pub state_schema_version: u32,
    pub seed: String,
    pub games_per_matchup: usize,
    pub samples_per_game: usize,
    pub battles_per_shard: usize,
    pub max_rounds: usize,
    pub eval_rq: f64,
    pub input_sha256: String,
    pub input_mode: String,
    pub team_sizes: Vec<usize>,
    pub matchups: Option<usize>,
    /// 可执行文件摘要同时约束引擎、schema、依赖版本和编译选项。
    pub executable_sha256: String,
    pub cases: Vec<input::Case>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// 拥有一份机制 state 的过程样本，标签与审计保持在特征之外。
///
/// frame=None 表示未推进初始状态，Some 表示已完成的可见帧；已决终局不作为特征样本。
/// winner_team_index 是最终唯一输入队伍索引，截断时为 None；progress 依赖最终长度，仅用于审计。
pub struct SampleRow {
    pub battle_id: u64,
    pub matchup_id: String,
    pub split: String,
    pub frame: Option<BattleModelFrame>,
    pub rounds_advanced: usize,
    /// 根据最终长度计算的采样进度，保存在 audit 通道。
    pub progress: f64,
    pub winner_team_index: Option<usize>,
    pub state: BattleModelState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// 每局对局身份、派生 seed、终止结果和实际样本计数；不复制逐帧机制 state。
///
/// battle_id 决定阵容和局次，split 由忽略队伍/成员顺序的 matchup_id 派生。
/// 截断可产生无标签样本，初始即终止的对局样本数为 0，不补造监督行。
pub struct BattleRow {
    pub battle_id: u64,
    pub case_index: usize,
    pub matchup_id: String,
    pub split: String,
    pub seed: String,
    pub outcome: BattleModelOutcome,
    pub samples: usize,
}

/// 同步执行子命令，将报告写到 stdout、进度写到 stderr，失败通过 Result 返回。
///
/// generate/bench/encode/build-manifest 会写文件，其他命令也可指定报告输出。
/// 生成或导出失败可能保留临时现场和已提交分片，不自动回滚其他成功阶段。
pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Generate(args) => generate::generate(&args),
        Command::Validate { out } => {
            let summary = validate::validate_dataset(&out)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            Ok(())
        }
        Command::Stats(args) => stats::run(&args),
        Command::Bench(args) => bench::run(&args),
        Command::Calibrate(args) => calibrate::run(&args),
        Command::BuildManifest(args) => manifest::run(&args),
        Command::Encode(args) => encode::run(&args),
    }
}

#[cfg(test)]
mod tests;

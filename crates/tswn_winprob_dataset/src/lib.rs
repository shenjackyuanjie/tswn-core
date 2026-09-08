//! 确定性的战斗状态生成、Parquet 分片与完整性校验。
mod bench;
mod generate;
mod input;
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
#[command(
    name = "tswn-winprob-dataset",
    about = "生成或校验战斗机制状态数据集"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Generate(GenerateArgs),
    Validate {
        #[arg(long)]
        out: PathBuf,
    },
    /// 汇总已完成数据集的分布统计，不修改数据。
    Stats(stats::StatsArgs),
    /// 采集生成与校验的规模基准，不修改数据。
    Bench(bench::BenchArgs),
}

#[derive(Debug, Clone, Args)]
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
pub struct SampleRow {
    pub battle_id: u64,
    pub matchup_id: String,
    pub split: String,
    pub frame: Option<BattleModelFrame>,
    pub rounds_advanced: usize,
    /// 审计用最终进度；不得作为模型特征。
    pub progress: f64,
    pub winner_team_index: Option<usize>,
    pub state: BattleModelState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleRow {
    pub battle_id: u64,
    pub case_index: usize,
    pub matchup_id: String,
    pub split: String,
    pub seed: String,
    pub outcome: BattleModelOutcome,
    pub samples: usize,
}

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
    }
}

#[cfg(test)]
mod tests;

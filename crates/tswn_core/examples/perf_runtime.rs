//! 只使用公开主 Runtime API 的 fixed30 / score benchmark 与 uProf 采样入口。
//! cargo run -p tswn_core --release --features no_debug --example perf_runtime -- --help

use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Parser, ValueEnum};
use tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::runtime::{
    RuntimeBatchSummary, RuntimeRunner, runtime_groups_win_rate, runtime_groups_win_rate_timed, runtime_score,
    runtime_score_timed,
};

#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    /// 每个文件是一场对局，保留原始队伍分隔，支持 FFA 和多队。
    WinRate,
    /// 对输入中的每一组分别评分。
    Score,
}

#[derive(Parser)]
struct Args {
    /// 原始 namerena 文本文件，或包含固定输入的目录。
    #[arg(short, long, default_value = "docs/perf/fixed_cases_30")]
    input: PathBuf,
    #[arg(long, value_enum, default_value = "win-rate")]
    mode: Mode,
    #[arg(short = 'n', long, default_value_t = 13_000, value_parser = clap::value_parser!(u32).range(1..))]
    runs: u32,
    /// 1 为单线程，0 为自动线程。
    #[arg(long, default_value_t = 1)]
    threads: u32,
    /// 额外开启逐场 init/fight 计时；uProf 采样时不要使用。
    #[arg(long)]
    timed: bool,
    /// score 随机对手的名字修饰符。
    #[arg(long, default_value = "!")]
    modifier: String,
}

fn input_files(input: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !input.is_dir() {
        return Ok(vec![input.to_owned()]);
    }
    let mut files = std::fs::read_dir(input)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    files.retain(|file| file.is_file() && file.extension().is_some_and(|ext| ext == "txt"));
    files.sort();
    Ok(files)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let files = input_files(&args.input)?;
    if files.is_empty() {
        return Err("benchmark 输入目录没有 txt 文件".into());
    }
    for file in files {
        let raw = std::fs::read_to_string(&file)?;
        let (groups, _) = RuntimeRunner::split_namerena_into_groups(raw);
        if groups.is_empty() {
            return Err(format!("benchmark 输入为空: {}", file.display()).into());
        }
        let n = args.runs as usize;
        let start = Instant::now();
        let summary = match args.mode {
            Mode::WinRate if args.timed => runtime_groups_win_rate_timed(&groups, n, WIN_RATE_EVAL_RQ, args.threads)?,
            Mode::WinRate => runtime_groups_win_rate(&groups, n, WIN_RATE_EVAL_RQ, args.threads)?,
            Mode::Score => {
                let mut combined = RuntimeBatchSummary::default();
                for group in &groups {
                    let part = if args.timed {
                        runtime_score_timed(group, &args.modifier, n, WIN_RATE_EVAL_RQ, args.threads)?
                    } else {
                        runtime_score(group, &args.modifier, n, WIN_RATE_EVAL_RQ, args.threads)?
                    };
                    combined.merge(part);
                }
                combined
            }
        };
        let elapsed = start.elapsed().as_secs_f64();
        println!(
            "{}",
            serde_json::json!({
                "input": file,
                "mode": match args.mode { Mode::WinRate => "win-rate", Mode::Score => "score" },
                "group_sizes": groups.iter().map(Vec::len).collect::<Vec<_>>(),
                "runs_per_matchup": n, "threads": args.threads, "timed": args.timed,
                "elapsed_seconds": elapsed, "total": summary.total, "wins": summary.wins,
                "errors": summary.errors, "guard_exhausted": summary.guard_exhausted,
                "init_nanos": summary.timing.init_nanos, "fight_nanos": summary.timing.fight_nanos,
            })
        );
        if summary.errors != 0 {
            return Err(format!("benchmark 发生 {} 次对局错误", summary.errors).into());
        }
    }
    Ok(())
}

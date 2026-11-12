//! 独立轮次区间的公开Runtime端到端计时；包含准备、初始化、执行与释放。
//! 输入IO/解析和JSON输出不计时；不使用PGO，不过滤guard。
use clap::Parser;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tswn_core::{
    namerena::eval_name::WIN_RATE_EVAL_RQ,
    runtime::{RuntimeRunner, prepared_runtime_win_rate_range},
};

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    input: PathBuf,
    /// 原始轮次起点（包含），默认与既有评估的 0..n 区间分离。
    #[arg(long, default_value_t = 2_000_000)]
    start: usize,
    /// 每个输入的局数。
    #[arg(short = 'n', long, default_value_t = 3000)]
    runs: usize,
    #[arg(long, default_value_t = 1)]
    threads: u32,
    #[arg(long, default_value = "win-rate")]
    mode: String,
}

fn input_files(input: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !input.is_dir() {
        return Ok(vec![input.to_owned()]);
    }
    let mut paths = std::fs::read_dir(input)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.is_file() && path.extension().is_some_and(|extension| extension == "txt"));
    paths.sort();
    Ok(paths)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.runs == 0 || args.threads != 1 || args.mode != "win-rate" {
        return Err("局数必须大于零，仅支持--threads 1 --mode win-rate".into());
    }
    let end = args.start.checked_add(args.runs).ok_or("轮次区间溢出")?;
    end.checked_add(tswn_core::runtime::PROFILE_START as usize).ok_or("seed编号溢出")?;
    let paths = input_files(&args.input)?;
    if paths.is_empty() {
        return Err("没有输入".into());
    }
    for path in paths {
        let raw = std::fs::read_to_string(&path)?;
        let (groups, seeds) = RuntimeRunner::split_namerena_into_groups(raw);
        if groups.is_empty() || !seeds.is_empty() {
            return Err("要求非空输入且不含文本seed；区间由--start指定".into());
        }
        let started = Instant::now();
        let summary = {
            let prepared = RuntimeRunner::prepare_groups_with_eval_rq(&groups, WIN_RATE_EVAL_RQ)?;
            prepared_runtime_win_rate_range(&prepared, args.start, end)?
        };
        let elapsed = started.elapsed().as_secs_f64();
        println!(
            "{}",
            serde_json::json!({
                "input": path, "mode": "win-rate", "threads": 1, "timed": false, "runs_per_matchup": args.runs,
                "elapsed_seconds": elapsed, "init_nanos": 0, "fight_nanos": 0, "start_inclusive": args.start, "end_exclusive": end,
                "group_sizes": groups.iter().map(Vec::len).collect::<Vec<_>>(), "total": summary.total,
                "wins": summary.wins, "errors": summary.errors, "guard_exhausted": summary.guard_exhausted,
            })
        );
        // 保护上限保持不变；guard 是有界结果，比较脚本须逐项核对。
        if summary.errors != 0 {
            return Err("对局出现初始化错误".into());
        }
    }
    Ok(())
}

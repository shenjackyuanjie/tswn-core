//! 仅用于 PGO 训练，显式执行与评估不重叠的原始轮次区间。
//! 输入准备及批量执行均使用公开 Runtime API；输出不用于性能比较。
use clap::Parser;
use std::path::{Path, PathBuf};
use tswn_core::{
    namerena::eval_name::WIN_RATE_EVAL_RQ,
    runtime::{RuntimeRunner, prepared_runtime_win_rate_range},
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    input: PathBuf,
    /// 原始轮次起点（包含），默认与评估的 0..n 区间分离。
    #[arg(long, default_value_t = 1_000_000)]
    start: usize,
    /// 每个训练输入的局数。
    #[arg(long, default_value_t = 10_000)]
    runs: usize,
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
    if args.runs == 0 || args.start < 1_000_000 {
        return Err("训练局数必须大于零，起点必须至少为 1000000".into());
    }
    let end = args.start.checked_add(args.runs).ok_or("训练轮次区间溢出")?;
    end.checked_add(tswn_core::runtime::PROFILE_START as usize).ok_or("训练 seed 编号溢出")?;
    let paths = input_files(&args.input)?;
    if paths.is_empty() {
        return Err("没有训练输入".into());
    }
    for path in paths {
        let raw = std::fs::read_to_string(&path)?;
        let (groups, _) = RuntimeRunner::split_namerena_into_groups(raw);
        let prepared = RuntimeRunner::prepare_groups_with_eval_rq(&groups, WIN_RATE_EVAL_RQ)?;
        let summary = prepared_runtime_win_rate_range(&prepared, args.start, end)?;
        println!(
            "{}",
            serde_json::json!({
                "input": path, "purpose": "pgo-training-only", "start_inclusive": args.start, "end_exclusive": end,
                "group_sizes": groups.iter().map(Vec::len).collect::<Vec<_>>(), "total": summary.total,
                "wins": summary.wins, "errors": summary.errors, "guard_exhausted": summary.guard_exhausted,
            })
        );
        // 保护上限保持不变；guard 是有界结果，构建脚本会与未插桩运行逐项核对。
        if summary.errors != 0 {
            return Err("训练出现初始化错误".into());
        }
    }
    Ok(())
}

//! 名竞 CLI 工具。
//!
//! 子命令：
//! - `fight`: 普通对战
//! - `bench auto`: 自动选择评分或胜率 benchmark
//! - `bench win-rate`: 显式双队胜率 benchmark
//! - `icon show|b64|save`: 图标预览与导出
//!
//! 示例：
//! ```bash
//! tswn-cli fight --raw "mario\nluigi\n\npeach\nbowser"
//! tswn-cli bench auto --raw "mario" -n 500 --perf
//! tswn-cli bench win-rate "mario" "luigi" -n 1000 -t 4
//! tswn-cli icon show mario luigi
//! ```

#[path = "tswn_cli/args.rs"]
mod args;
#[path = "tswn_cli/bench.rs"]
mod bench;
#[path = "tswn_cli/fight.rs"]
mod fight;
#[path = "tswn_cli/icon.rs"]
mod icon;

use args::ParsedCommand;

fn print_banner() {
    println!("欢迎来到 tswn - {}, 使用 --help/-h 获取帮助信息谢谢喵", tswn_core::version());
    println!("WARNING: ALPHA 版本, 仅供测试使用, 已知有 bug, 暂未实现: 天卫、Boss、武器");
    println!("发现行为不一致请不要惊慌, 呼叫 shenjack 即可 (qq: 3695888)（欢迎入群 hack: 935216900）");
}

fn main() {
    let cli = args::parse().unwrap_or_else(|err| err.exit());

    if !matches!(cli.command, ParsedCommand::Fight { out_raw: true, .. }) {
        print_banner();
    }

    match cli.command {
        ParsedCommand::Fight { raw, out_raw } => fight::run(raw, out_raw),
        ParsedCommand::BenchAuto {
            raw,
            n,
            mode,
            threads,
            perf,
        } => bench::run_benchmark(&raw, n, mode, threads, perf),
        ParsedCommand::BenchWinRate {
            team1,
            team2,
            n,
            mode,
            threads,
            perf,
            keep_rq,
        } => {
            let eval_rq = if keep_rq {
                tswn_core::player::eval_name::DEFAULT_EVAL_RQ
            } else {
                tswn_core::player::eval_name::WIN_RATE_EVAL_RQ
            };
            let raw = format!("{team1}\n\n{team2}");
            bench::run_bench_winrate(&raw, n, mode, threads, eval_rq, perf);
        }
        ParsedCommand::BenchGroupWinRate {
            target,
            against,
            n,
            mode,
            threads,
            perf,
            keep_rq,
        } => {
            let eval_rq = if keep_rq {
                tswn_core::player::eval_name::DEFAULT_EVAL_RQ
            } else {
                tswn_core::player::eval_name::WIN_RATE_EVAL_RQ
            };
            bench::run_bench_group_win_rate(&target, &against, n, mode, threads, eval_rq, perf);
        }
        ParsedCommand::IconShow { names } => icon::print_icons(&names),
        ParsedCommand::IconB64 { names } => {
            if let Err(err) = icon::print_icon_b64(&names) {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
        ParsedCommand::IconSave { dir, names } => {
            if let Err(err) = icon::save_icons(&dir, &names) {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
    }
}

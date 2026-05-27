//! tswn-cli 的主入口与命令分发。
//!
//! 这个文件本身只负责三件事：
//! - 调用 `args::parse()` 解析并归一化命令行输入；
//! - 为交互式命令打印欢迎 banner；
//! - 把解析结果分发到 `fight`、`bench`、`icon`、`to_diy` 等实现模块。
//!
//! 相关模块概览：
//! - `tswn_cli/args/`: `clap` 参数定义、输入来源统一、子命令映射。
//! - `tswn_cli/fight/`: 普通对战、raw 输出、diff 输出。
//! - `tswn_cli/bench/`: 评分 / 胜率基准测试、批量评估、`namer-pf`。
//! - `tswn_cli/icon.rs`: 图标预览、base64 导出、文件保存。
//! - `tswn_cli/to_diy.rs`: DIY / OL 覆盖文本导出。
//!
//! 现在入口改成目录下的 `main.rs`，门面模块改成各自目录里的 `mod.rs`，
//! 编译器会按 Rust 默认模块规则自动解析子模块，因此不再需要 `#[path = ...]`。
//!
//! 顶层命令概览：
//! - `fight`: 运行普通对战，可选 `--out-raw` 输出聚合战斗日志。
//! - `raw`: 直接运行 namerena 原始输入，兼容普通对战和 `!test!` 基准测试输入。
//! - `diff`: 运行普通对战，并按 runner diff 格式输出。
//! - `bench auto`: 按输入组数自动切换评分基准测试或胜率基准测试。
//! - `bench win-rate`: 显式比较两队胜率。
//! - `bench group-win-rate`: 目标组对多个对手组逐个统计并汇总平均胜率。
//! - `bench batch-rate` / `bench cqp`: 批量计算选手组对靶子组列表的平均胜率。
//! - `bench pair`: 评估选手与队友二人组的 top-head 胜率和。
//! - `namer-pf`: 输出与 ica-plugin `/namer-pf` 对齐的四项评分，可用 `--mode` 只跑指定项。
//! - `icon show|b64|save`: 预览、导出或保存玩家图标。
//! - `to-diy`: 将名字导出为 DIY/OL overlay 格式。
//!
//! 输出约定：
//! - 默认会打印欢迎 banner，便于交互式使用。
//! - `fight --out-raw`、`raw`、`diff`、`namer-pf` 会跳过 banner，避免污染机器可读输出。
//!
//! 输入约定：
//! - 原始对战/benchmark 输入使用 namerena raw 文本，组与组之间用空行分隔。
//! - `bench batch-rate` / `bench cqp` 使用文件列表时，每行一组，组内用 `+` 分隔多个名字。
//! - `to-diy` 单号模式接收一个名字，文件模式按行批量处理。
//!
//! 示例：
//! ```bash
//! tswn-cli fight -r "mario\nluigi\n\npeach\nbowser"
//! tswn-cli raw -r "mario\nluigi\n\npeach\nbowser"
//! tswn-cli diff -r "mario\nluigi\n\npeach\nbowser"
//! tswn-cli bench auto -r "mario" -n 10000 --perf
//! tswn-cli bench win-rate "mario" "luigi" -n 10000 -t 4
//! tswn-cli bench group-win-rate -l "mario" -a "luigi" -a "peach" -n 10000
//! tswn-cli bench cqp -l targets.txt -p players.txt --min-screen 60.5
//! tswn-cli bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 3
//! tswn-cli namer-pf -r "mario\nluigi"
//! tswn-cli namer-pf -r "mario\nluigi" --mode pp qd
//! tswn-cli to-diy -r "mario@team+fire" -o diy.txt
//! tswn-cli icon show mario luigi
//! ```

mod args;
mod bench;
mod fight;
mod icon;
mod to_diy;

use args::ParsedCommand;

/// 小批量 benchmark 不值得承担线程调度开销，超过该阈值后再考虑并行路径。
pub(crate) const BENCH_PARALLEL_THRESHOLD: usize = 64;

fn print_banner() {
    println!("欢迎来到 tswn - {}, 使用 --help/-h 获取帮助信息谢谢喵", tswn_core::version());
    println!("WARNING: ALPHA 版本, 仅供测试使用, 已知有 bug, 暂未实现: 天卫、Boss、武器");
    println!("发现行为不一致请不要惊慌, 呼叫 shenjack 即可 (qq: 3695888)（欢迎入群 hack: 935216900）");
}

fn main() {
    let cli = args::parse().unwrap_or_else(|err| err.exit());

    if !matches!(
        cli.command,
        ParsedCommand::Fight { out_raw: true, .. }
            | ParsedCommand::FightRaw { .. }
            | ParsedCommand::FightDiff { .. }
            | ParsedCommand::NamerPf { .. }
    ) {
        print_banner();
    }

    match cli.command {
        ParsedCommand::Fight { raw, out_raw } => fight::run(raw, out_raw),
        ParsedCommand::FightDiff { raw } => fight::run_diff(raw),
        ParsedCommand::FightRaw { raw, n, threads } => fight::run_raw(raw, n, threads),
        ParsedCommand::BenchAuto {
            raw,
            n,
            mode,
            threads,
            perf,
            buckets_step,
        } => bench::run_benchmark(&raw, n, mode, threads, perf, buckets_step),
        ParsedCommand::BenchWinRate {
            team1,
            team2,
            n,
            mode,
            threads,
            perf,
            keep_rq,
            buckets_step,
        } => {
            let eval_rq = if keep_rq {
                tswn_core::player::eval_name::DEFAULT_EVAL_RQ
            } else {
                tswn_core::player::eval_name::WIN_RATE_EVAL_RQ
            };
            let raw = format!("{team1}\n\n{team2}");
            bench::run_bench_winrate(&raw, n, mode, threads, eval_rq, perf, buckets_step);
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
        ParsedCommand::BenchBatchRate {
            target_groups,
            player_groups,
            player_labels,
            n,
            mode,
            threads,
            perf,
            keep_rq,
            verbose,
            out_file,
            force,
            log,
            pure,
            min_screen,
            min_file,
            wr_precision,
        } => {
            let eval_rq = if keep_rq {
                tswn_core::player::eval_name::DEFAULT_EVAL_RQ
            } else {
                tswn_core::player::eval_name::WIN_RATE_EVAL_RQ
            };
            bench::run_bench_batch_rate(
                &target_groups,
                &player_groups,
                &player_labels,
                n,
                mode,
                threads,
                eval_rq,
                verbose,
                perf,
                out_file.as_deref(),
                force,
                log,
                pure,
                min_screen,
                min_file,
                wr_precision,
            );
        }
        ParsedCommand::BenchPair {
            target_groups,
            players,
            teammates,
            head,
            n,
            mode,
            threads,
            perf,
            keep_rq,
            verbose,
            out_file,
            force,
            log,
            pure,
            min_screen,
            min_file,
            wr_precision,
        } => {
            let eval_rq = if keep_rq {
                tswn_core::player::eval_name::DEFAULT_EVAL_RQ
            } else {
                tswn_core::player::eval_name::WIN_RATE_EVAL_RQ
            };
            bench::run_bench_pair(
                &target_groups,
                &players,
                &teammates,
                head,
                n,
                mode,
                threads,
                eval_rq,
                verbose,
                perf,
                out_file.as_deref(),
                force,
                log,
                pure,
                min_screen,
                min_file,
                wr_precision,
            );
        }
        ParsedCommand::NamerPf { raw, n, threads, modes } => bench::run_namer_pf(&raw, n, threads, &modes),
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
        ParsedCommand::ToDiy {
            names,
            from_file,
            out_file,
            old,
            minions,
        } => to_diy::run(&names, from_file, out_file.as_deref(), old, minions),
    }
}

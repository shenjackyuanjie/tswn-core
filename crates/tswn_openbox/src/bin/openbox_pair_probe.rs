//! OpenBox `pair` 后端的 headless 基准入口。
//!
//! OpenBox 的 `pair` 平时只能在 GUI 里触发：GUI 调 `backend::run_pair`，后者把
//! 「选手 × 队友 × 靶子」矩阵交给 `tswn_core` 的共享 CQP/CQD 调度器。要比较两个提交的
//! pair 路径，需要一个不依赖窗口的入口，本 binary 就是这个入口。
//!
//! 输出约定（做 A/B 时依赖这两条）：
//!
//! - **stdout**：该路径产生的日志行，按原顺序逐行打印。屏幕日志里没有耗时字段，
//!   因此同一份输入在新旧提交上的 stdout 应当逐字节一致，可以直接取哈希比较；
//! - **stderr**：一行摘要，形如
//!   `probe elapsed_s=... lines=... progress_ticks=... progress_last=... progress_total=... matrix_estimate=... done=...`。
//!   `elapsed_s` 是整批 `run_pair` 的墙钟，不含进程启动与结果打印。
//!
//! 用法：
//!
//! ```powershell
//! cargo run --release -p tswn_openbox --bin openbox_pair_probe -- `
//!   --players docs/perf/cqp/sqp6000_first20.txt `
//!   --teammates crates/tswn_openbox/assets/teammates/teammate_fz.txt `
//!   --targets crates/tswn_openbox/assets/targets/target2.txt `
//!   --count 100 --threads auto --head 5
//! ```
//!
//! 线程口径与 `openbox_mem_probe` 一致：`--threads auto`（或 `0`）走自动线程，
//! 显式给数字则按给定线程数；`count` 等价于 GUI 的 1% / 10% / 100%（100 / 1000 / 10000）。
//! `keep_rq` 固定为 `true`，与 `openbox_mem_probe` 的批量口径保持一致。
//! 复测口径、派生输入与历史结果见 `docs/perf/guides/openbox-pair-probe.md`。

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use tswn_openbox::backend::{CommonBenchOptions, OutputMode, PairDetailMode, PairInput, ProgressEvent, run_pair};

const USAGE: &str = "\
用法: openbox_pair_probe [选项]

  --players <FILE>             选手列表，每行一个组合（必填）
  --teammates <FILE>           队友列表，每行一个组合（必填）
  --targets <FILE>             靶子列表（必填）
  --count <N>                  每个 matchup 的场数，默认 100
  --threads <N|auto|0>         线程数，auto/0 为自动线程，默认 auto
  --head <N>                   每名选手保留最高的 N 个队友组合，默认 5
  --detail <none|top|every>    屏幕日志的 cqp 明细模式，默认 top
  --teammate-factored          队友文件按带权 TOML（[[targets]]）解析
  --target-factored            靶子文件按带权 TOML（[[targets]]）解析
  -h, --help                   打印本说明
";

struct Args {
    players: PathBuf,
    teammates: PathBuf,
    targets: PathBuf,
    count: usize,
    threads: Option<usize>,
    head: usize,
    detail_mode: PairDetailMode,
    teammate_factored: bool,
    target_factored: bool,
}

impl Args {
    fn parse() -> Result<Option<Self>, String> {
        let mut args = std::env::args().skip(1);
        let mut parsed = Self {
            players: PathBuf::from("docs/perf/cqp/sqp6000_first20.txt"),
            teammates: PathBuf::from("crates/tswn_openbox/assets/teammates/teammate_fz.txt"),
            targets: PathBuf::from("crates/tswn_openbox/assets/targets/target2.txt"),
            count: 100,
            threads: None,
            head: 5,
            detail_mode: PairDetailMode::Top,
            teammate_factored: false,
            target_factored: false,
        };
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} 需要一个取值"));
            match arg.as_str() {
                "-h" | "--help" => {
                    print!("{USAGE}");
                    return Ok(None);
                }
                "--players" => parsed.players = PathBuf::from(value("--players")?),
                "--teammates" => parsed.teammates = PathBuf::from(value("--teammates")?),
                "--targets" => parsed.targets = PathBuf::from(value("--targets")?),
                "--count" => parsed.count = value("--count")?.parse().map_err(|err| format!("--count 取值非法: {err}"))?,
                "--threads" => {
                    let raw = value("--threads")?;
                    parsed.threads = match raw.as_str() {
                        "auto" | "0" => None,
                        _ => Some(raw.parse().map_err(|err| format!("--threads 取值非法: {err}"))?),
                    };
                }
                "--head" => parsed.head = value("--head")?.parse().map_err(|err| format!("--head 取值非法: {err}"))?,
                "--detail" => {
                    parsed.detail_mode = match value("--detail")?.as_str() {
                        "none" => PairDetailMode::None,
                        "top" => PairDetailMode::Top,
                        "every" => PairDetailMode::Every,
                        other => return Err(format!("--detail 只支持 none/top/every，收到: {other}")),
                    }
                }
                "--teammate-factored" => parsed.teammate_factored = true,
                "--target-factored" => parsed.target_factored = true,
                other => return Err(format!("未知参数: {other}（用 --help 查看用法）")),
            }
        }
        Ok(Some(parsed))
    }
}

fn read_all(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| format!("读取失败 {}: {err}", path.display()))
}

/// 按输入文件的“组”数估算矩阵规模。
///
/// 带权 TOML 按 `[[targets]]` 块计数，普通文本按非空行计数。真实 matchup 数还要减去
/// 镜像与重名跳过的项，所以调用方只会把它当成量级参考。
fn group_count(path: &Path, factored: bool) -> Result<usize, String> {
    let content = read_all(path)?;
    let count = if factored {
        content.lines().filter(|line| line.trim_start().starts_with("[[targets]]")).count()
    } else {
        content.lines().filter(|line| !line.trim().is_empty()).count()
    };
    Ok(count)
}

fn main() {
    let args = match Args::parse() {
        Ok(Some(args)) => args,
        Ok(None) => return,
        Err(err) => {
            eprintln!("openbox_pair_probe: {err}");
            std::process::exit(2);
        }
    };

    let (players, teammates, targets) = match (read_all(&args.players), read_all(&args.teammates), read_all(&args.targets)) {
        (Ok(players), Ok(teammates), Ok(targets)) => (players, teammates, targets),
        (players, teammates, targets) => {
            for err in [players, teammates, targets].into_iter().filter_map(Result::err) {
                eprintln!("openbox_pair_probe: {err}");
            }
            std::process::exit(2);
        }
    };
    let matrix = match (
        group_count(&args.players, false),
        group_count(&args.teammates, args.teammate_factored),
        group_count(&args.targets, args.target_factored),
    ) {
        (Ok(players), Ok(teammates), Ok(targets)) => Some((players, teammates, targets)),
        _ => None,
    };

    let cancel = Arc::new(AtomicBool::new(false));
    let input = PairInput {
        target_text: targets,
        target_factor_enabled: args.target_factored,
        player_text: players,
        player_double_plus: false,
        teammate_text: teammates,
        teammate_double_plus: false,
        teammate_factor_enabled: args.teammate_factored,
        head: args.head,
        detail_mode: args.detail_mode,
        detail_min: None,
        highlight_delta: None,
        output_mode: OutputMode::Log,
        output_file: None,
        options: CommonBenchOptions {
            count: args.count,
            threads: args.threads,
            keep_rq: true,
            verbose: false,
            min_screen: None,
            min_file: None,
            wr_precision: 3,
        },
        cancel: Arc::clone(&cancel),
    };

    let lines = RefCell::new(Vec::<String>::new());
    let ticks = Cell::new(0usize);
    let last = Cell::new(0usize);
    let total = Cell::new(0usize);
    let done = RefCell::new(None::<Result<String, String>>);
    let start = Instant::now();
    run_pair(input, |event| match event {
        ProgressEvent::Log(line) | ProgressEvent::HighlightLog(line) | ProgressEvent::SkillBoardLog(line) => {
            lines.borrow_mut().push(line);
        }
        ProgressEvent::Progress { done, total: all } => {
            ticks.set(ticks.get() + 1);
            last.set(done);
            total.set(all);
        }
        ProgressEvent::Done(result) => *done.borrow_mut() = Some(result),
    });
    let elapsed = start.elapsed().as_secs_f64();

    let lines = lines.into_inner();
    for line in &lines {
        println!("{line}");
    }
    let status = match done.into_inner() {
        Some(Ok(message)) => message,
        Some(Err(err)) => {
            eprintln!("openbox_pair_probe: pair 执行失败: {err}");
            std::process::exit(3);
        }
        None => {
            eprintln!("openbox_pair_probe: pair 没有返回结果");
            std::process::exit(3);
        }
    };
    let matrix = matrix.map_or_else(
        || "unknown".to_owned(),
        |(players, teammates, targets)| {
            format!(
                "{}x{}x{}={}",
                players,
                teammates,
                targets,
                players.saturating_mul(teammates).saturating_mul(targets)
            )
        },
    );
    eprintln!(
        "probe elapsed_s={elapsed:.6} lines={} progress_ticks={} progress_last={} progress_total={} matrix_estimate={matrix} done={status}",
        lines.len(),
        ticks.get(),
        last.get(),
        total.get(),
    );
}

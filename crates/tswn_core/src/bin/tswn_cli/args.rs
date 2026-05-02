use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchThreadMode {
    /// 自动或显式并行运行 benchmark。
    Parallel,
    /// 强制使用单线程运行 benchmark。
    SingleThread,
}

#[derive(Debug)]
pub enum ParsedCommand {
    Fight {
        /// 普通对战输入，使用 namerena raw 格式。
        raw: String,
        /// 是否改为输出 raw 聚合战斗日志。
        out_raw: bool,
    },
    FightDiff {
        /// 普通对战输入，使用 namerena raw 格式，并按 runner diff 的格式输出。
        raw: String,
    },
    FightRaw {
        /// 原始 namerena 输入，可能是普通对战，也可能是 `!test!` benchmark 输入。
        raw: String,
        /// 评分或胜率测试的模拟场数。
        n: usize,
        /// 显式指定的 benchmark 线程数。
        threads: Option<usize>,
    },
    BenchAuto {
        /// benchmark 原始输入，按组数自动分流到评分或胜率测试。
        raw: String,
        /// benchmark 模拟场数。
        n: usize,
        /// benchmark 线程模式。
        mode: BenchThreadMode,
        /// 显式指定的 benchmark 线程数。
        threads: Option<usize>,
        /// 是否输出 total/init/fight 耗时统计。
        perf: bool,
        /// 分段输出步长（每 N 场输出一次累积胜率）。
        buckets_step: Option<usize>,
    },
    BenchWinRate {
        /// 第一队输入，格式与普通输入中的单组相同。
        team1: String,
        /// 第二队输入，格式与普通输入中的单组相同。
        team2: String,
        /// 每组对局的模拟场数。
        n: usize,
        /// benchmark 线程模式。
        mode: BenchThreadMode,
        /// 显式指定的 benchmark 线程数。
        threads: Option<usize>,
        /// 是否输出 total/init/fight 耗时统计。
        perf: bool,
        /// 是否保持 `rq=4`，不模拟 JS `win_rate` 对 `rq` 的污染。
        keep_rq: bool,
        /// 分段输出步长（每 N 场输出一次累积胜率）。
        buckets_step: Option<usize>,
    },
    BenchGroupWinRate {
        /// 靶子组输入，格式与普通输入中的单组相同。
        target: String,
        /// 对手组列表，每项都支持单人或整组输入。
        against: Vec<String>,
        /// 每组对局的模拟场数。
        n: usize,
        /// benchmark 线程模式。
        mode: BenchThreadMode,
        /// 显式指定的 benchmark 线程数。
        threads: Option<usize>,
        /// 是否输出 total/init/fight 耗时统计。
        perf: bool,
        /// 是否保持 `rq=4`，不模拟 JS `win_rate` 对 `rq` 的污染。
        keep_rq: bool,
    },
    BenchBatchRate {
        /// 靶子组列表；每项都已从 `+` 分隔行转换成 `\n` 分隔的 namerena 组字符串。
        target_groups: Vec<String>,
        /// 选手组列表；每项都已从 `+` 分隔行转换成 `\n` 分隔的 namerena 组字符串。
        player_groups: Vec<String>,
        /// 选手组展示标签，保留文件中的原始行文本。
        player_labels: Vec<String>,
        /// 每组对局的模拟场数。
        n: usize,
        /// benchmark 线程模式。
        mode: BenchThreadMode,
        /// 显式指定的 benchmark 线程数。
        threads: Option<usize>,
        /// 是否输出 total/init/fight 耗时统计。
        perf: bool,
        /// 是否输出逐个靶子的明细胜率。
        verbose: bool,
        /// 批量结果输出文件；未指定时输出到标准输出。
        out_file: Option<PathBuf>,
        /// 若输出文件已存在，是否直接覆盖而不再确认。
        force: bool,
        /// 是否保持 `rq=4`，不模拟 JS `win_rate` 对 `rq` 的污染。
        keep_rq: bool,
        /// 最低胜率阈值 (0-10000)；仅在终端显示平均胜率不低于此值的选手。
        min_wr: Option<u16>,
    },
    IconShow {
        /// 要展示图标的玩家名字列表。
        names: Vec<String>,
    },
    IconB64 {
        /// 要导出 base64 PNG 的玩家名字列表。
        names: Vec<String>,
    },
    IconSave {
        /// 图标输出目录。
        dir: PathBuf,
        /// 要保存图标的玩家名字列表。
        names: Vec<String>,
    },
}

#[derive(Debug)]
pub struct ParsedCli {
    /// 解析完成后的 CLI 命令。
    pub command: ParsedCommand,
}

#[derive(Debug, Parser)]
#[command(
    name = "tswn-cli",
    about = "名竞 CLI 工具",
    version = env!("CARGO_PKG_VERSION"),
    disable_help_subcommand = true,
    subcommand_required = true,
    arg_required_else_help = true
)]
struct Cli {
    /// 顶层 CLI 子命令。
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// 运行普通对战。
    ///
    /// 示例:
    ///   tswn-cli fight
    ///   tswn-cli fight -r "mario\nluigi\n\npeach\nbowser"
    ///   tswn-cli fight --out-raw -f input.txt
    #[command(verbatim_doc_comment)]
    Fight(FightCommand),
    /// 运行原始 namerena 对战，并直接输出 raw 聚合战斗日志。
    ///
    /// 示例:
    ///   tswn-cli raw
    ///   tswn-cli raw -r "mario\nluigi\n\npeach\nbowser"
    ///   tswn-cli raw -f input.txt
    #[command(name = "raw", verbatim_doc_comment)]
    FightRaw(FightRawCommand),
    /// 运行普通对战，并按 runner diff 的格式输出。
    ///
    /// 示例:
    ///   tswn-cli diff
    ///   tswn-cli diff -r "mario\nluigi\n\npeach\nbowser"
    ///   tswn-cli diff -f input.txt
    #[command(name = "diff", verbatim_doc_comment)]
    FightDiff(FightDiffCommand),
    /// 运行 benchmark 相关功能。
    Bench(BenchCommand),
    /// 玩家图标相关功能。
    Icon(IconCommand),
}

#[derive(Debug, Args)]
struct FightCommand {
    /// 普通对战输入来源参数。
    #[command(flatten)]
    input: InputArgs,

    /// 输出 raw 聚合战斗日志。
    #[arg(long)]
    out_raw: bool,
}

#[derive(Debug, Args)]
struct FightRawCommand {
    /// 原始对战输入来源参数。
    #[command(flatten)]
    input: InputArgs,

    /// 评分对局数量。
    #[arg(
        short = 'n',
        long = "count",
        default_value_t = 10000,
        value_name = "N"
    )]
    count: usize,

    /// 指定 benchmark 线程数。
    #[arg(short = 't', long = "thread", value_parser = parse_thread_count, value_name = "N")]
    thread: Option<usize>,
}

#[derive(Debug, Args)]
struct FightDiffCommand {
    /// 原始对战输入来源参数。
    #[command(flatten)]
    input: InputArgs,
}

#[derive(Debug, Args)]
struct BenchCommand {
    /// benchmark 子命令。
    #[command(subcommand)]
    command: BenchSubcommand,
}

#[derive(Debug, Subcommand)]
enum BenchSubcommand {
    /// 自动检测输入组数并运行评分或胜率测试。
    ///
    /// 1 组输入会跑评分，2 组及以上输入会跑胜率。
    ///
    /// 示例:
    ///   tswn-cli bench auto
    ///   tswn-cli bench auto -r "mario" -n 10000 --perf
    ///   tswn-cli bench auto -f input.txt -n 10000 -t 8
    #[command(verbatim_doc_comment)]
    Auto(BenchAutoCommand),
    /// 显式运行两队胜率测试。
    ///
    /// 示例:
    ///   tswn-cli bench win-rate "mario" "luigi" -n 10000
    ///   tswn-cli bench win-rate "mario" "luigi" --keep-rq --perf
    #[command(name = "win-rate", verbatim_doc_comment)]
    WinRate(BenchWinRateCommand),
    /// 显式运行目标组对多个对手组的胜率测试，并汇总平均胜率。
    ///
    /// `--against` 可重复传入，每项都支持单人或整组输入。
    ///
    /// 示例:
    ///   tswn-cli bench group-win-rate -l "mario\nluigi" --against "bowser" --against "peach\ndaisy"
    ///   tswn-cli bench group-win-rate -l "mario" -a "luigi" -a "peach" -n 10000 --perf
    #[command(name = "group-win-rate", verbatim_doc_comment)]
    GroupWinRate(BenchGroupWinRateCommand),
    /// 批量计算选手列表对靶子列表的平均胜率 (cqp = 丛擎跑)。
    ///
    /// `cqp` 与 `batch-rate` 是同一个命令的两个名字，功能完全相同。
    ///
    /// 靶子文件和选手文件每行一组，组内用 + 分隔，跳过空行。
    /// `--out-file` 会输出 JSONL，每个选手组一行结果。
    /// `--min-wr` 可设置最低胜率阈值 (0-10000)，仅在终端显示达标选手。
    ///
    /// 示例:
    ///   tswn-cli bench batch-rate -l targets.txt -p players.txt -n 10000 -t 8
    ///   tswn-cli bench cqp -l targets.txt -p players.txt -n 10000 -t 8
    ///   tswn-cli bench cqp -l targets.txt -p players.txt -n 10000 -m 5000
    ///   tswn-cli bench batch-rate -l targets.txt -p players.txt -o result.jsonl
    #[command(
        name = "batch-rate",
        visible_alias = "cqp",
        verbatim_doc_comment
    )]
    BatchRate(BenchBatchRateCommand),
}

#[derive(Debug, Args)]
struct BenchAutoCommand {
    /// benchmark 输入来源参数。
    #[command(flatten)]
    input: InputArgs,

    /// benchmark 公共参数。
    #[command(flatten)]
    options: BenchOptions,
}

#[derive(Debug, Args)]
struct BenchWinRateCommand {
    /// 队伍 1，格式与普通输入中的单组相同。
    team1: String,

    /// 队伍 2，格式与普通输入中的单组相同。
    team2: String,

    /// 胜率 benchmark 公共参数。
    #[command(flatten)]
    options: BenchOptions,

    /// 保持 rq=4，不模拟 JS win_rate 对 rq 的污染。
    #[arg(long)]
    keep_rq: bool,
}

#[derive(Debug, Args)]
struct BenchGroupWinRateCommand {
    /// 目标组，格式与普通输入中的单组相同；支持 `-l/--target`。
    #[arg(short = 'l', long = "target", value_name = "TARGET")]
    target: String,

    /// 对手组，可重复传入；每项支持单人或整组输入。
    #[arg(
        short = 'a',
        long = "against",
        required = true,
        value_name = "GROUP"
    )]
    against: Vec<String>,

    /// 组胜率 benchmark 公共参数。
    #[command(flatten)]
    options: BenchOptions,

    /// 保持 rq=4，不模拟 JS win_rate 对 rq 的污染。
    #[arg(long)]
    keep_rq: bool,
}

#[derive(Debug, Args)]
struct BenchBatchRateCommand {
    /// 靶子列表文件，每行一组，组内用 + 分隔，跳过空行；支持 `-l/--target-list`。
    #[arg(short = 'l', long = "target-list", value_name = "FILE")]
    target_list: PathBuf,

    /// 选手列表文件，每行一组，组内用 + 分隔，跳过空行；支持 `-p/--player-list`。
    #[arg(short = 'p', long = "player-list", value_name = "FILE")]
    player_list: PathBuf,

    /// 批量胜率测试的公共 benchmark 参数。
    #[command(flatten)]
    options: BenchOptions,

    /// 显示逐个靶子的明细胜率。
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// 将批量结果写入指定文件；格式为 JSONL，每个选手组输出一行结果。
    #[arg(short = 'o', long = "out-file", value_name = "FILE")]
    out_file: Option<PathBuf>,

    /// 若输出文件已存在，则直接覆盖，不再交互确认。
    #[arg(short = 'f', long = "force", requires = "out_file")]
    force: bool,

    /// 保持 rq=4，不模拟 JS win_rate 对 rq 的污染。
    #[arg(long)]
    keep_rq: bool,

    /// 最低胜率阈值 (0-10000)，仅在终端显示平均胜率不低于此值的选手。
    ///
    /// 例如 5000 表示仅显示胜率 ≥ 50% 的选手。文件输出不受此参数影响。
    #[arg(short = 'm', long = "min-wr", value_parser = parse_min_wr, value_name = "N")]
    min_wr: Option<u16>,
}

#[derive(Debug, Args)]
struct BenchOptions {
    /// 运行场数。
    #[arg(
        short = 'n',
        long = "count",
        default_value_t = 10000,
        value_name = "N"
    )]
    count: usize,

    /// 使用单线程运行；支持 `-s/--single-thread`。
    #[arg(short = 's', long, conflicts_with = "thread")]
    single_thread: bool,

    /// 指定 benchmark 线程数。
    #[arg(short = 't', long = "thread", value_parser = parse_thread_count, value_name = "N")]
    thread: Option<usize>,

    /// 输出 total/init/fight 耗时统计。
    #[arg(long)]
    perf: bool,

    /// 分段输出累积胜率，每隔 N 场输出一次（如 --buckets-step 1000）。
    /// 分段模式下强制单线程以保证顺序正确。
    #[arg(long = "buckets-step", value_name = "N")]
    buckets_step: Option<usize>,
}

#[derive(Debug, Args)]
struct InputArgs {
    /// 使用提供的原始字符串作为输入，支持 `\n` 换行；支持 `-r/--raw`。
    #[arg(
        short = 'r',
        long,
        conflicts_with = "file",
        value_name = "STRING"
    )]
    raw: Option<String>,

    /// 从文件读取输入；支持 `-f/--file`。
    #[arg(
        short = 'f',
        long,
        conflicts_with = "raw",
        value_name = "FILE"
    )]
    file: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct IconCommand {
    /// 图标相关子命令。
    #[command(subcommand)]
    command: IconSubcommand,
}

#[derive(Debug, Subcommand)]
enum IconSubcommand {
    /// 输出玩家图标信息和终端渲染预览。
    ///
    /// 示例:
    ///   tswn-cli icon show mario luigi
    #[command(verbatim_doc_comment)]
    Show(IconNames),
    /// 输出图标的 base64 PNG 数据 URL。
    ///
    /// 需要启用 `png_render` feature。
    ///
    /// 示例:
    ///   tswn-cli icon b64 mario
    #[command(name = "b64", verbatim_doc_comment)]
    B64(IconNames),
    /// 将图标 PNG 保存到指定目录。
    ///
    /// 需要启用 `png_render` feature。
    ///
    /// 示例:
    ///   tswn-cli icon save ./icons mario luigi
    #[command(name = "save", verbatim_doc_comment)]
    Save(IconSaveCommand),
}

#[derive(Debug, Args)]
struct IconNames {
    /// 玩家名字列表。
    #[arg(required = true, value_name = "NAME")]
    names: Vec<String>,
}

#[derive(Debug, Args)]
struct IconSaveCommand {
    /// 输出目录。
    dir: PathBuf,

    /// 玩家名字列表。
    #[arg(required = true, value_name = "NAME")]
    names: Vec<String>,
}

/// 解析命令行参数，并转换成内部使用的结构化命令。
pub fn parse() -> Result<ParsedCli, clap::Error> {
    let cli = Cli::try_parse()?;
    ParsedCli::from_cli(cli)
}

impl ParsedCli {
    /// 将 `clap` 解析结果转换成更适合执行阶段使用的内部命令结构。
    fn from_cli(cli: Cli) -> Result<Self, clap::Error> {
        let command = match cli.command {
            CliCommand::Fight(cmd) => ParsedCommand::Fight {
                raw: cmd.input.read_or_stdin()?,
                out_raw: cmd.out_raw,
            },
            CliCommand::FightRaw(cmd) => ParsedCommand::FightRaw {
                raw: cmd.input.read_or_stdin()?,
                n: cmd.count.max(1),
                threads: cmd.thread,
            },
            CliCommand::FightDiff(cmd) => ParsedCommand::FightDiff {
                raw: cmd.input.read_or_stdin()?,
            },
            CliCommand::Bench(BenchCommand { command }) => match command {
                BenchSubcommand::Auto(cmd) => ParsedCommand::BenchAuto {
                    raw: cmd.input.read_or_stdin()?,
                    n: cmd.options.count.max(1),
                    mode: cmd.options.mode(),
                    threads: cmd.options.thread,
                    perf: cmd.options.perf,
                    buckets_step: cmd.options.buckets_step,
                },
                BenchSubcommand::WinRate(cmd) => ParsedCommand::BenchWinRate {
                    team1: cmd.team1,
                    team2: cmd.team2,
                    n: cmd.options.count.max(1),
                    mode: cmd.options.mode(),
                    threads: cmd.options.thread,
                    perf: cmd.options.perf,
                    keep_rq: cmd.keep_rq,
                    buckets_step: cmd.options.buckets_step,
                },
                BenchSubcommand::GroupWinRate(cmd) => ParsedCommand::BenchGroupWinRate {
                    target: decode_raw(&cmd.target),
                    against: cmd.against.into_iter().map(|value| decode_raw(&value)).collect(),
                    n: cmd.options.count.max(1),
                    mode: cmd.options.mode(),
                    threads: cmd.options.thread,
                    perf: cmd.options.perf,
                    keep_rq: cmd.keep_rq,
                },
                BenchSubcommand::BatchRate(cmd) => {
                    let target_content = read_file(&cmd.target_list)?;
                    let target_groups = parse_plus_separated_groups(&target_content);
                    let player_content = read_file(&cmd.player_list)?;
                    let (player_groups, player_labels) = parse_plus_separated_groups_with_labels(&player_content);
                    ParsedCommand::BenchBatchRate {
                        target_groups,
                        player_groups,
                        player_labels,
                        n: cmd.options.count.max(1),
                        mode: cmd.options.mode(),
                        threads: cmd.options.thread,
                        perf: cmd.options.perf,
                        verbose: cmd.verbose,
                        out_file: cmd.out_file,
                        force: cmd.force,
                        keep_rq: cmd.keep_rq,
                        min_wr: cmd.min_wr,
                    }
                }
            },
            CliCommand::Icon(IconCommand { command }) => match command {
                IconSubcommand::Show(cmd) => ParsedCommand::IconShow { names: cmd.names },
                IconSubcommand::B64(cmd) => ParsedCommand::IconB64 { names: cmd.names },
                IconSubcommand::Save(cmd) => ParsedCommand::IconSave {
                    dir: cmd.dir,
                    names: cmd.names,
                },
            },
        };
        Ok(Self { command })
    }
}

impl BenchOptions {
    /// 根据 `--single-thread` 与 `--thread` 参数计算 benchmark 线程模式。
    fn mode(&self) -> BenchThreadMode {
        if self.single_thread {
            BenchThreadMode::SingleThread
        } else {
            BenchThreadMode::Parallel
        }
    }
}

impl InputArgs {
    /// 按 `--raw`、`--file` 或 stdin 的优先级读取输入内容。
    fn read_or_stdin(&self) -> Result<String, clap::Error> {
        match (&self.raw, &self.file) {
            (Some(raw), None) => Ok(decode_raw(raw)),
            (None, Some(path)) => read_file(path),
            (None, None) => read_stdin(),
            _ => Err(cli_error("输入来源只能使用一种")),
        }
    }
}

/// 从标准输入读取完整的 namerena 原始输入。
fn read_stdin() -> Result<String, clap::Error> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|err| cli_error(format!("读取 stdin 失败: {err}")))?;
    let raw = strip_utf8_bom(&raw).to_string();
    if raw.trim().is_empty() {
        return Err(cli_error("未提供 raw_namerena 输入"));
    }
    Ok(raw)
}

/// 从指定文件读取完整文本输入。
fn read_file(path: &Path) -> Result<String, clap::Error> {
    let content = fs::read_to_string(path).map_err(|err| cli_error(format!("读取文件失败: {err}")))?;
    Ok(strip_utf8_bom(&content).to_string())
}

/// 将命令行里的字面量 `\n` 还原成真实换行。
fn decode_raw(raw: &str) -> String { raw.replace("\\n", "\n") }

/// 解析并校验 benchmark 线程数参数。
fn parse_thread_count(raw: &str) -> Result<usize, String> {
    let value = raw.parse::<usize>().map_err(|_| "线程数必须是正整数".to_string())?;
    if value == 0 {
        Err("线程数必须大于 0".to_string())
    } else {
        Ok(value)
    }
}

/// 解析并校验最低胜率阈值参数 (0-10000)。
fn parse_min_wr(raw: &str) -> Result<u16, String> {
    let value = raw.parse::<u16>().map_err(|_| "阈值必须是 0-10000 之间的整数".to_string())?;
    if value > 10000 {
        Err("阈值必须不超过 10000".to_string())
    } else {
        Ok(value)
    }
}

/// 构造统一风格的 CLI 参数校验错误。
fn cli_error(message: impl Into<String>) -> clap::Error { Cli::command().error(ErrorKind::ValueValidation, message.into()) }

/// 去除 UTF-8 BOM (U+FEFF) 前缀。
fn strip_utf8_bom(s: &str) -> &str { s.strip_prefix('\u{feff}').unwrap_or(s) }

/// 解析“每个非空行都是一个 `+` 分隔组”的文件内容。
/// 返回转换后的 namerena 组字符串列表，组内成员之间用 `\n` 分隔。
fn parse_plus_separated_groups(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.split('+').map(str::trim).collect::<Vec<_>>().join("\n"))
        .collect()
}

/// 与 `parse_plus_separated_groups` 相同，但会额外保留每行原始文本作为展示标签。
fn parse_plus_separated_groups_with_labels(content: &str) -> (Vec<String>, Vec<String>) {
    let mut groups = Vec::new();
    let mut labels = Vec::new();
    for line in content.lines().map(str::trim).filter(|line| !line.is_empty()) {
        labels.push(line.to_string());
        groups.push(line.split('+').map(str::trim).collect::<Vec<_>>().join("\n"));
    }
    (groups, labels)
}

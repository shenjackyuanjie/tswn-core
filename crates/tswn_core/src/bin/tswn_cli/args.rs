use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchThreadMode {
    Parallel,
    SingleThread,
}

#[derive(Debug)]
pub enum ParsedCommand {
    Fight {
        raw: String,
        out_raw: bool,
    },
    FightRaw {
        raw: String,
        n: usize,
        threads: Option<usize>,
    },
    BenchAuto {
        raw: String,
        n: usize,
        mode: BenchThreadMode,
        threads: Option<usize>,
        perf: bool,
    },
    BenchWinRate {
        team1: String,
        team2: String,
        n: usize,
        mode: BenchThreadMode,
        threads: Option<usize>,
        perf: bool,
        keep_rq: bool,
    },
    BenchGroupWinRate {
        target: String,
        against: Vec<String>,
        n: usize,
        mode: BenchThreadMode,
        threads: Option<usize>,
        perf: bool,
        keep_rq: bool,
    },
    IconShow {
        names: Vec<String>,
    },
    IconB64 {
        names: Vec<String>,
    },
    IconSave {
        dir: PathBuf,
        names: Vec<String>,
    },
}

#[derive(Debug)]
pub struct ParsedCli {
    pub command: ParsedCommand,
}

#[derive(Debug, Parser)]
#[command(
    name = "tswn-cli",
    about = "名竞 CLI 工具",
    disable_help_subcommand = true,
    subcommand_required = true,
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// 运行普通对战。
    ///
    /// 示例:
    ///   tswn-cli fight
    ///   tswn-cli fight --raw "mario\nluigi\n\npeach\nbowser"
    ///   tswn-cli fight --out-raw --file input.txt
    #[command(verbatim_doc_comment)]
    Fight(FightCommand),
    /// 运行原始 namerena 对战，并直接输出 raw 聚合战斗日志。
    ///
    /// 示例:
    ///   tswn-cli raw
    ///   tswn-cli raw --raw "mario\nluigi\n\npeach\nbowser"
    ///   tswn-cli raw --file input.txt
    #[command(name = "raw", verbatim_doc_comment)]
    FightRaw(FightRawCommand),
    /// 运行 benchmark 相关功能。
    Bench(BenchCommand),
    /// 玩家图标相关功能。
    Icon(IconCommand),
}

#[derive(Debug, Args)]
struct FightCommand {
    #[command(flatten)]
    input: InputArgs,

    /// 输出 raw 聚合战斗日志。
    #[arg(long)]
    out_raw: bool,
}

#[derive(Debug, Args)]
struct FightRawCommand {
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
struct BenchCommand {
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
    ///   tswn-cli bench auto --raw "mario" -n 10000 --perf
    ///   tswn-cli bench auto --file input.txt -n 10000 -t 8
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
    ///   tswn-cli bench group-win-rate --target "mario\nluigi" --against "bowser" --against "peach\ndaisy"
    ///   tswn-cli bench group-win-rate --target "mario" -a "luigi" -a "peach" -n 10000 --perf
    #[command(name = "group-win-rate", verbatim_doc_comment)]
    GroupWinRate(BenchGroupWinRateCommand),
}

#[derive(Debug, Args)]
struct BenchAutoCommand {
    #[command(flatten)]
    input: InputArgs,

    #[command(flatten)]
    options: BenchOptions,
}

#[derive(Debug, Args)]
struct BenchWinRateCommand {
    /// 队伍 1，格式与普通输入中的单组相同。
    team1: String,

    /// 队伍 2，格式与普通输入中的单组相同。
    team2: String,

    #[command(flatten)]
    options: BenchOptions,

    /// 保持 rq=4，不模拟 JS win_rate 对 rq 的污染。
    #[arg(long)]
    keep_rq: bool,
}

#[derive(Debug, Args)]
struct BenchGroupWinRateCommand {
    /// 目标组，格式与普通输入中的单组相同。
    #[arg(long, value_name = "TARGET")]
    target: String,

    /// 对手组，可重复传入；每项支持单人或整组输入。
    #[arg(
        short = 'a',
        long = "against",
        required = true,
        value_name = "GROUP"
    )]
    against: Vec<String>,

    #[command(flatten)]
    options: BenchOptions,

    /// 保持 rq=4，不模拟 JS win_rate 对 rq 的污染。
    #[arg(long)]
    keep_rq: bool,
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

    /// 使用单线程运行。
    #[arg(long, conflicts_with = "thread")]
    single_thread: bool,

    /// 指定 benchmark 线程数。
    #[arg(short = 't', long = "thread", value_parser = parse_thread_count, value_name = "N")]
    thread: Option<usize>,

    /// 输出 total/init/fight 耗时统计。
    #[arg(long)]
    perf: bool,
}

#[derive(Debug, Args)]
struct InputArgs {
    /// 使用提供的原始字符串作为输入，支持 `\n` 换行。
    #[arg(long, conflicts_with = "file", value_name = "STRING")]
    raw: Option<String>,

    /// 从文件读取输入。
    #[arg(long, conflicts_with = "raw", value_name = "FILE")]
    file: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct IconCommand {
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

pub fn parse() -> Result<ParsedCli, clap::Error> {
    let cli = Cli::try_parse()?;
    ParsedCli::from_cli(cli)
}

impl ParsedCli {
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
            CliCommand::Bench(BenchCommand { command }) => match command {
                BenchSubcommand::Auto(cmd) => ParsedCommand::BenchAuto {
                    raw: cmd.input.read_or_stdin()?,
                    n: cmd.options.count.max(1),
                    mode: cmd.options.mode(),
                    threads: cmd.options.thread,
                    perf: cmd.options.perf,
                },
                BenchSubcommand::WinRate(cmd) => ParsedCommand::BenchWinRate {
                    team1: cmd.team1,
                    team2: cmd.team2,
                    n: cmd.options.count.max(1),
                    mode: cmd.options.mode(),
                    threads: cmd.options.thread,
                    perf: cmd.options.perf,
                    keep_rq: cmd.keep_rq,
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
    fn mode(&self) -> BenchThreadMode {
        if self.single_thread {
            BenchThreadMode::SingleThread
        } else {
            BenchThreadMode::Parallel
        }
    }
}

impl InputArgs {
    fn read_or_stdin(&self) -> Result<String, clap::Error> {
        match (&self.raw, &self.file) {
            (Some(raw), None) => Ok(decode_raw(raw)),
            (None, Some(path)) => read_file(path),
            (None, None) => read_stdin(),
            _ => Err(cli_error("输入来源只能使用一种")),
        }
    }
}

fn read_stdin() -> Result<String, clap::Error> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|err| cli_error(format!("读取 stdin 失败: {err}")))?;
    if raw.trim().is_empty() {
        return Err(cli_error("未提供 raw_namerena 输入"));
    }
    Ok(raw)
}

fn read_file(path: &Path) -> Result<String, clap::Error> {
    fs::read_to_string(path).map_err(|err| cli_error(format!("读取文件失败: {err}")))
}

fn decode_raw(raw: &str) -> String { raw.replace("\\n", "\n") }

fn parse_thread_count(raw: &str) -> Result<usize, String> {
    let value = raw.parse::<usize>().map_err(|_| "线程数必须是正整数".to_string())?;
    if value == 0 {
        Err("线程数必须大于 0".to_string())
    } else {
        Ok(value)
    }
}

fn cli_error(message: impl Into<String>) -> clap::Error { Cli::command().error(ErrorKind::ValueValidation, message.into()) }

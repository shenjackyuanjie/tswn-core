//! `clap` 命令树与“从 `clap` 到内部命令”的转换层。
//!
//! 这个文件保留两类内容：
//! - 命令树本身，也就是用户在终端里能看到的 CLI 形状；
//! - 把 `clap` 解析结果收口成 `ParsedCommand` 的归一化逻辑。
//!
//! 这样做的重点是把“外部交互形状”和“执行阶段的内部模型”分开：
//! `clap` 结构体需要围绕帮助文案、别名、冲突参数、默认值来设计；执行阶段则更关心
//! 输入是否已经读好、文件是否已经展开、线程模式是否已经统一。

use std::path::PathBuf;

#[cfg(test)]
use std::path::Path;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

use super::input::{
    cli_error, decode_raw, parse_line_list, parse_non_negative_f64, parse_percent_0_100, parse_player_groups_with_labels,
    parse_plus_separated_groups, parse_positive_usize, parse_thread_count, parse_to_diy_file_names, parse_wr_precision,
    read_file, read_stdin,
};
use super::parsed::{BenchThreadMode, NamerPfMode, ParsedCli, ParsedCommand};

// ----------------------------------------------------------------------------
// 顶层 CLI 结构。
// ----------------------------------------------------------------------------

#[derive(Debug, Parser)]
#[command(
    name = "tswn-cli",
    about = "名竞 CLI 工具",
    version = env!("CARGO_PKG_VERSION"),
    disable_help_subcommand = true,
    subcommand_required = true,
    arg_required_else_help = true
)]
pub(super) struct Cli {
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
    /// 运行基准测试相关功能。
    Bench(BenchCommand),
    /// 运行与 ica-plugin `/namer-pf` 相同的四项评分。
    #[command(name = "namer-pf", verbatim_doc_comment)]
    NamerPf(NamerPfCommand),
    /// 玩家图标相关功能。
    Icon(IconCommand),
    /// 将名字转换为 DIY / OL 覆盖格式。
    ///
    /// 默认接收一个名字并输出详细信息；单号用 `-r/--raw NAME`，文件批量用 `-f/--file FILE`。
    /// 文件模式会按行读取多个名字，跳过空行，并按输入顺序逐行输出导出结果。
    /// 默认输出 `+ol` 形式；`--old` 切换为旧版 `+diy` 形式。
    /// `--minions` 会在 `+ol` 中附带幻影/使魔/丧尸模板，方便继续 DIY 它们的属性和技能。
    /// `-o/--out-file FILE` 可将输出写入文件。
    ///
    /// 示例:
    ///   tswn-cli to-diy -r "mario@team+fire"
    ///   tswn-cli to-diy -f names.txt
    ///   tswn-cli to-diy -r "mario@team+fire" --old
    ///   tswn-cli to-diy -r "地狱之轮 #mW88BamWo@Shabby_fish" --minions
    ///   tswn-cli to-diy -r "mario@team+fire" -o diy.txt
    ///   tswn-cli to-diy --file names.txt --out-file diy.txt
    #[command(name = "to-diy", verbatim_doc_comment)]
    ToDiy(ToDiyCommand),
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

    /// 指定基准测试线程数。
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
    /// 基准测试子命令。
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
    /// `--out-file` 默认输出 `winrate<space>name`；`--log` 切到 JSONL，`--pure` 切到仅名字。
    /// `--min-screen` 控制终端显示阈值；`--min-file` 控制文件写入阈值（均为 0~100）。
    ///
    /// 示例:
    ///   tswn-cli bench batch-rate -l targets.txt -p players.txt -n 10000 -t 8
    ///   tswn-cli bench cqp -l targets.txt -p players.txt -n 10000 -t 8
    ///   tswn-cli bench cqp -l targets.txt -p players.txt --min-screen 60.5
    ///   tswn-cli bench batch-rate -l targets.txt -p players.txt -o result.txt --min-file 65
    ///   tswn-cli bench batch-rate -l targets.txt -p players.txt -o result.jsonl --log
    #[command(
        name = "batch-rate",
        visible_alias = "cqp",
        verbatim_doc_comment
    )]
    BatchRate(BenchBatchRateCommand),
    /// 为每个选手和 teammate-list 中的每个队友组成二人组，计算各组合 batch rate 后取最高 head 个求和。
    ///
    /// player-list 和 teammate-list 均为每行一个名字；player-list 不支持 `--player-list-double-plus`。
    ///
    /// 示例:
    ///   tswn-cli bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 3 -n 10000
    ///   tswn-cli bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 5 -o result.txt
    #[command(name = "pair", verbatim_doc_comment)]
    Pair(BenchPairCommand),
}

#[derive(Debug, Args)]
struct BenchAutoCommand {
    /// 基准测试输入来源参数。
    #[command(flatten)]
    input: InputArgs,

    /// 基准测试公共参数。
    #[command(flatten)]
    options: BenchOptions,
}

#[derive(Debug, Args)]
struct BenchWinRateCommand {
    /// 队伍 1，格式与普通输入中的单组相同。
    team1: String,

    /// 队伍 2，格式与普通输入中的单组相同。
    team2: String,

    /// 胜率测试公共参数。
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

    /// 组胜率测试公共参数。
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

    /// 使用 ++ 分隔 player-list 中的组内成员，避免拆开名字里的 +diy[...] / +ol:...。
    #[arg(long = "player-list-double-plus")]
    player_list_double_plus: bool,

    /// 批量胜率测试的公共基准测试参数。
    #[command(flatten)]
    options: BenchOptions,

    /// 显示逐个靶子的明细胜率。
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// 将批量结果写入指定文件。
    #[arg(short = 'o', long = "out-file", value_name = "FILE")]
    out_file: Option<PathBuf>,

    /// 若输出文件已存在，则直接覆盖，不再交互确认。
    #[arg(short = 'f', long = "force", requires = "out_file")]
    force: bool,

    /// 保持 rq=4，不模拟 JS win_rate 对 rq 的污染。
    #[arg(long)]
    keep_rq: bool,

    /// 仅在输出到文件时生效：输出 JSONL。
    #[arg(long = "log", requires = "out_file", conflicts_with = "pure")]
    log: bool,

    /// 仅在输出到文件时生效：每行只输出 `name`。
    #[arg(long = "pure", requires = "out_file", conflicts_with = "log")]
    pure: bool,

    /// 仅在终端显示平均胜率不低于此值的选手（0~100）。
    #[arg(long = "min-screen", value_parser = parse_percent_0_100, value_name = "N")]
    min_screen: Option<f64>,

    /// 仅在输出到文件时生效：只写入平均胜率不低于此值的选手（0~100）。
    #[arg(long = "min-file", requires = "out_file", value_parser = parse_percent_0_100, value_name = "N")]
    min_file: Option<f64>,

    /// 胜率保留小数位数（默认 3）。
    #[arg(long = "wr-precision", default_value_t = 3, value_parser = parse_wr_precision, value_name = "N")]
    wr_precision: usize,
}

#[derive(Debug, Args)]
struct BenchPairCommand {
    /// 靶子列表文件，每行一组，组内用 + 分隔，跳过空行；支持 `-l/--target-list`。
    #[arg(short = 'l', long = "target-list", value_name = "FILE")]
    target_list: PathBuf,

    /// 选手列表文件，每行一个名字，跳过空行；支持 `-p/--player-list`。
    #[arg(short = 'p', long = "player-list", value_name = "FILE")]
    player_list: PathBuf,

    /// 队友列表文件，每行一个名字，跳过空行。
    #[arg(long = "teammate-list", value_name = "FILE")]
    teammate_list: PathBuf,

    /// 每名选手取最高的 N 个二人组 batch rate 求和。
    #[arg(long = "head", value_parser = parse_positive_usize, value_name = "N")]
    head: usize,

    /// `pair` 测试的公共基准测试参数。
    #[command(flatten)]
    options: BenchOptions,

    /// 显示逐个队友和靶子的明细胜率。
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// 将结果写入指定文件。
    #[arg(short = 'o', long = "out-file", value_name = "FILE")]
    out_file: Option<PathBuf>,

    /// 若输出文件已存在，则直接覆盖，不再交互确认。
    #[arg(short = 'f', long = "force", requires = "out_file")]
    force: bool,

    /// 保持 rq=4，不模拟 JS win_rate 对 rq 的污染。
    #[arg(long)]
    keep_rq: bool,

    /// 仅在输出到文件时生效：输出 JSONL。
    #[arg(long = "log", requires = "out_file", conflicts_with = "pure")]
    log: bool,

    /// 仅在输出到文件时生效：每行只输出 `name`。
    #[arg(long = "pure", requires = "out_file", conflicts_with = "log")]
    pure: bool,

    /// 仅在终端显示最终分数不低于此值的选手。
    #[arg(long = "min-screen", value_parser = parse_non_negative_f64, value_name = "N")]
    min_screen: Option<f64>,

    /// 仅在输出到文件时生效：只写入最终分数不低于此值的选手。
    #[arg(long = "min-file", requires = "out_file", value_parser = parse_non_negative_f64, value_name = "N")]
    min_file: Option<f64>,

    /// 胜率保留小数位数（默认 3）。
    #[arg(long = "wr-precision", default_value_t = 3, value_parser = parse_wr_precision, value_name = "N")]
    wr_precision: usize,
}

#[derive(Debug, Args)]
struct NamerPfCommand {
    /// 输入来源参数；每行一个名字组，组内可用 `+` 分隔。
    #[command(flatten)]
    input: InputArgs,

    /// 每个评分项的运行场数。
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

    /// 只运行指定评分项；可重复传入或一次传多个，不传则运行 pp/pd/qp/qd 全部四项。
    #[arg(long = "mode", value_enum, value_name = "MODE", num_args = 1.., value_delimiter = ',', action = ArgAction::Append)]
    mode: Vec<NamerPfModeArg>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum NamerPfModeArg {
    Pp,
    Pd,
    Qp,
    Qd,
}

impl From<NamerPfModeArg> for NamerPfMode {
    fn from(value: NamerPfModeArg) -> Self {
        match value {
            NamerPfModeArg::Pp => Self::Pp,
            NamerPfModeArg::Pd => Self::Pd,
            NamerPfModeArg::Qp => Self::Qp,
            NamerPfModeArg::Qd => Self::Qd,
        }
    }
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

/// 通用输入来源。
///
/// 这层只描述“原始输入从哪里来”，并不负责决定最终业务含义。
/// 真正的读取优先级在 `read_or_stdin()` 里统一收口。
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

#[derive(Debug, Args)]
struct ToDiyCommand {
    /// 玩家名字（namerena raw 格式）。
    ///
    /// 支持 @ 队伍名和 + 武器名。使用 --file 时不可同时传 RAW。
    #[arg(
        short = 'r',
        long = "raw",
        value_name = "NAME",
        required_unless_present = "file",
        conflicts_with = "file"
    )]
    raw: Option<String>,

    /// 从文件按行读取多个玩家名字；空行会被跳过，输出也按行对应。
    #[arg(
        short = 'f',
        long = "file",
        value_name = "FILE",
        conflicts_with = "raw"
    )]
    file: Option<PathBuf>,

    /// 将结果写入指定文件；未指定时输出到标准输出。
    #[arg(short = 'o', long = "out-file", value_name = "FILE")]
    out_file: Option<PathBuf>,

    /// 输出旧版 `+diy` 形式；默认输出 `+ol` 形式。
    #[arg(long = "old")]
    old: bool,

    /// 在 `+ol` 中附带幻影/使魔/丧尸模板；旧版 `+diy` 无法表达这些字段。
    #[arg(
        long = "minions",
        alias = "with-minions",
        conflicts_with = "old"
    )]
    minions: bool,
}

/// 解析命令行参数，并转换成内部使用的结构化命令。
pub fn parse() -> Result<ParsedCli, clap::Error> {
    let cli = Cli::try_parse()?;
    ParsedCli::from_cli(cli)
}

impl ParsedCli {
    /// 将 `clap` 解析结果转换成更适合执行阶段使用的内部命令结构。
    ///
    /// 这一层是 CLI 的“边界层”：
    /// - 外部世界仍然是 `clap` 风格的多个可选字段；
    /// - 进入执行层之后，就全部变成已经归一化、可直接消费的结构。
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
                    let (player_groups, player_labels) =
                        parse_player_groups_with_labels(&player_content, cmd.player_list_double_plus);
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
                        log: cmd.log,
                        pure: cmd.pure,
                        min_screen: cmd.min_screen,
                        min_file: cmd.min_file,
                        wr_precision: cmd.wr_precision,
                    }
                }
                BenchSubcommand::Pair(cmd) => {
                    let target_content = read_file(&cmd.target_list)?;
                    let target_groups = parse_plus_separated_groups(&target_content);
                    let player_content = read_file(&cmd.player_list)?;
                    let teammate_content = read_file(&cmd.teammate_list)?;
                    ParsedCommand::BenchPair {
                        target_groups,
                        players: parse_line_list(&player_content),
                        teammates: parse_line_list(&teammate_content),
                        head: cmd.head,
                        n: cmd.options.count.max(1),
                        mode: cmd.options.mode(),
                        threads: cmd.options.thread,
                        perf: cmd.options.perf,
                        verbose: cmd.verbose,
                        out_file: cmd.out_file,
                        force: cmd.force,
                        keep_rq: cmd.keep_rq,
                        log: cmd.log,
                        pure: cmd.pure,
                        min_screen: cmd.min_screen,
                        min_file: cmd.min_file,
                        wr_precision: cmd.wr_precision,
                    }
                }
            },
            CliCommand::NamerPf(cmd) => ParsedCommand::NamerPf {
                raw: cmd.input.read_or_stdin()?,
                n: cmd.count.max(1),
                threads: cmd.thread,
                modes: normalize_namer_pf_modes(&cmd.mode),
            },
            CliCommand::Icon(IconCommand { command }) => match command {
                IconSubcommand::Show(cmd) => ParsedCommand::IconShow { names: cmd.names },
                IconSubcommand::B64(cmd) => ParsedCommand::IconB64 { names: cmd.names },
                IconSubcommand::Save(cmd) => ParsedCommand::IconSave {
                    dir: cmd.dir,
                    names: cmd.names,
                },
            },
            CliCommand::ToDiy(cmd) => {
                let (names, from_file) = match (cmd.raw, cmd.file) {
                    (Some(name), None) => (vec![name], false),
                    (None, Some(path)) => (parse_to_diy_file_names(&read_file(&path)?)?, true),
                    _ => return Err(cli_error("to-diy 只能使用 --raw/NAME 或 --file 其中一种输入")),
                };
                ParsedCommand::ToDiy {
                    names,
                    from_file,
                    out_file: cmd.out_file,
                    old: cmd.old,
                    minions: cmd.minions,
                }
            }
        };
        Ok(Self { command })
    }
}

fn normalize_namer_pf_modes(modes: &[NamerPfModeArg]) -> Vec<NamerPfMode> {
    if modes.is_empty() {
        return NamerPfMode::ALL.to_vec();
    }

    NamerPfMode::ALL
        .into_iter()
        .filter(|mode| modes.iter().any(|arg| NamerPfMode::from(*arg) == *mode))
        .collect()
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
    ///
    /// 这里把三种来源统一成一个字符串，执行阶段完全不必感知输入来自哪里。
    fn read_or_stdin(&self) -> Result<String, clap::Error> {
        match (&self.raw, &self.file) {
            (Some(raw), None) => Ok(decode_raw(raw)),
            (None, Some(path)) => read_file(path),
            (None, None) => read_stdin(),
            _ => Err(cli_error("输入来源只能使用一种")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_diy_command_accepts_raw_out_file_and_old_flag() {
        let cli = Cli::try_parse_from(["tswn-cli", "to-diy", "-r", "mario@team", "-o", "out.txt", "--old"]).unwrap();
        match cli.command {
            CliCommand::ToDiy(cmd) => {
                assert_eq!(cmd.raw.as_deref(), Some("mario@team"));
                assert_eq!(cmd.file, None);
                assert_eq!(cmd.out_file.as_deref(), Some(Path::new("out.txt")));
                assert!(cmd.old);
                assert!(!cmd.minions);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn to_diy_command_accepts_minions_flag() {
        let cli = Cli::try_parse_from(["tswn-cli", "to-diy", "-r", "mario@team+shadow", "--minions"]).unwrap();
        match cli.command {
            CliCommand::ToDiy(cmd) => {
                assert_eq!(cmd.raw.as_deref(), Some("mario@team+shadow"));
                assert!(cmd.minions);
                assert!(!cmd.old);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn to_diy_command_rejects_old_with_minions() {
        let err = Cli::try_parse_from(["tswn-cli", "to-diy", "-r", "mario", "--old", "--minions"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn namer_pf_accepts_multiple_modes() {
        let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario", "--mode", "pp", "qd"]).unwrap();
        match cli.command {
            CliCommand::NamerPf(cmd) => {
                assert_eq!(cmd.mode, vec![NamerPfModeArg::Pp, NamerPfModeArg::Qd]);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn namer_pf_defaults_to_all_modes() {
        let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario"]).unwrap();
        let parsed = ParsedCli::from_cli(cli).unwrap();
        match parsed.command {
            ParsedCommand::NamerPf { modes, .. } => {
                assert_eq!(modes, NamerPfMode::ALL.to_vec());
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn batch_rate_rejects_log_and_pure_together() {
        let err = Cli::try_parse_from([
            "tswn-cli",
            "bench",
            "batch-rate",
            "-l",
            "targets.txt",
            "-p",
            "players.txt",
            "-o",
            "out.txt",
            "--log",
            "--pure",
        ])
        .unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn batch_rate_accepts_min_screen_and_min_file() {
        let cli = Cli::try_parse_from([
            "tswn-cli",
            "bench",
            "batch-rate",
            "-l",
            "targets.txt",
            "-p",
            "players.txt",
            "--min-screen",
            "66.5",
            "-o",
            "out.txt",
            "--min-file",
            "70",
        ])
        .unwrap();
        match cli.command {
            CliCommand::Bench(BenchCommand {
                command: BenchSubcommand::BatchRate(cmd),
            }) => {
                assert_eq!(cmd.min_screen, Some(66.5));
                assert_eq!(cmd.min_file, Some(70.0));
                assert_eq!(cmd.wr_precision, 3);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn batch_rate_accepts_wr_precision() {
        let cli = Cli::try_parse_from([
            "tswn-cli",
            "bench",
            "batch-rate",
            "-l",
            "targets.txt",
            "-p",
            "players.txt",
            "--wr-precision",
            "5",
        ])
        .unwrap();
        match cli.command {
            CliCommand::Bench(BenchCommand {
                command: BenchSubcommand::BatchRate(cmd),
            }) => assert_eq!(cmd.wr_precision, 5),
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn pair_accepts_required_args_and_wr_precision() {
        let cli = Cli::try_parse_from([
            "tswn-cli",
            "bench",
            "pair",
            "-l",
            "targets.txt",
            "-p",
            "players.txt",
            "--teammate-list",
            "teammates.txt",
            "--head",
            "3",
            "--wr-precision",
            "4",
        ])
        .unwrap();
        match cli.command {
            CliCommand::Bench(BenchCommand {
                command: BenchSubcommand::Pair(cmd),
            }) => {
                assert_eq!(cmd.head, 3);
                assert_eq!(cmd.wr_precision, 4);
                assert_eq!(cmd.teammate_list, PathBuf::from("teammates.txt"));
            }
            _ => panic!("unexpected command"),
        }
    }
}

//! `clap` 命令树与“从 `clap` 到内部命令”的转换层。
//!
//! 这个文件保留两类内容：
//! - 命令树本身，也就是用户在终端里能看到的 CLI 形状；
//! - 把 `clap` 解析结果收口成 `ParsedCommand` 的归一化逻辑。
//!
//! 这样做的重点是把“外部交互形状”和“执行阶段的内部模型”分开：
//! `clap` 结构体需要围绕帮助文案、别名、冲突参数、默认值来设计；执行阶段则更关心
//! 输入是否已经读好、文件是否已经展开、线程模式是否已经统一。

use std::collections::HashSet;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use super::input::{
    cli_error, decode_raw, parse_factored_target_groups, parse_metric_spec, parse_non_negative_f64, parse_percent_0_100,
    parse_player_groups_with_labels, parse_plus_separated_groups, parse_positive_usize, parse_thread_count,
    parse_to_diy_file_names, parse_win_rate_teams, parse_wr_precision, read_file, read_stdin,
};
use super::parsed::{BenchThreadMode, NamerPfMetric, NamerPfMetricSpec, PairDetailMode, ParsedCli, ParsedCommand};

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
    ///   tswn-cli fight -f input.txt
    #[command(verbatim_doc_comment)]
    Fight(FightCommand),
    /// 运行 runtime 相关调试/迁移入口。
    #[command(name = "runtime", verbatim_doc_comment)]
    Runtime(RuntimeCommand),
    /// 运行基准测试相关功能。
    Bench(BenchCommand),
    /// 运行与 ica-plugin `/namer-pf` 相同的五项评分（pp/pd/qp/qd/sum），可选技能榜。
    ///
    /// 每行一个名字组，组内用 `+` 分隔。默认五项全部输出到屏幕，屏幕行格式为
    /// `名字组合 指标:分数`，例如 `mario+luigi pp:12345`。
    ///
    /// `--metric SPEC` 可重复传入，语法为 `NAME[:MIN_SCREEN[:FILE[:MIN_FILE]]]`：
    /// - NAME：pp / pd / qp / qd / sum 之一（sum = 其余四项之和，需四项全算）；
    /// - MIN_SCREEN：屏幕输出阈值（分），不低于才打印到屏幕；
    /// - FILE：该指标的输出文件，每行格式为 `分数 名字组合`；
    /// - MIN_FILE：文件写入阈值（分）。
    ///
    /// 空段表示跳过该项，例如 `pp::pp.txt` 表示不设屏幕阈值但写入 pp.txt。
    /// FILE 路径不要包含 `:`（Windows 盘符前缀如 `C:\out.txt` 除外）。
    /// 指标固定按 pp/pd/qp/qd/sum 顺序输出，与传入顺序无关；同一指标或同一输出文件
    /// 只能出现一次。配合 `--no-screen` 可只写文件不打印屏幕。
    ///
    /// `--skill-board FILE` 开启技能榜：FILE 是阈值 TOML，形如
    /// `[sklfire]` / `[sklice]` 小节写 pp/qp/qd/all 阈值，`[lessskl]` 写白板号阈值。
    /// 程序把每个名字导出 `+diy`、取熟练度最高的技能，分数超过该技能阈值时输出
    /// `技能名指标 分数 名字`（如 `冰冻qp 6647 mario`）；全部技能等级小于 30 时改按
    /// `[lessskl]` 阈值；`全能` 行还需同时满足 pp>=8000、pd>=9000、qp>=6000、qd>=7000。
    /// 开启技能榜会强制计算全部四项评分。
    ///
    /// 示例:
    ///   tswn-cli namer-pf -r "mario"
    ///   tswn-cli namer-pf -f names.txt --metric sum --metric pp:8000
    ///   tswn-cli namer-pf -f names.txt --metric pp:8000:pp.txt:7500 --no-screen
    ///   tswn-cli namer-pf -f names.txt --skill-board score_now.toml --skill-board-out board.txt
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
    /// 单号模式在未指定 `-o` 且名字不含 `+` 时，会附加输出一段原始信息详情
    /// （名字/队伍/八围/技能/name_factor）；`--no-details` 可关闭。文件批量模式
    /// 不输出详情。
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
    /// 逐条输出 initial / frame / result JSON，并立即刷新 stdout。
    #[arg(long)]
    jsonl: bool,
    /// Runtime main_round 的总调用预算（空轮次也计数）。
    #[arg(long, default_value_t = 20_000, value_parser = parse_positive_usize, value_name = "N")]
    max_rounds: usize,
}

#[derive(Debug, Args)]
struct FightDiffCommand {
    /// 原始对战输入来源参数。
    #[command(flatten)]
    input: InputArgs,
}

#[derive(Debug, Args)]
struct RuntimeCommand {
    /// runtime 子命令。
    #[command(subcommand)]
    command: RuntimeSubcommand,
}

#[derive(Debug, Subcommand)]
enum RuntimeSubcommand {
    /// 按主 Runtime 诊断格式输出对局。
    #[command(name = "diff")]
    Diff(FightDiffCommand),
    /// 使用默认 custom runtime profile 运行 raw 输入，并输出 normalized-run JSON。
    ///
    /// 示例:
    ///   tswn-cli runtime normalized-run -r "left\n\nright" --max-rounds 8
    ///   tswn-cli runtime normalized-run -f input.txt
    #[command(name = "normalized-run", verbatim_doc_comment)]
    NormalizedRun(RuntimeNormalizedRunCommand),
}

#[derive(Debug, Args)]
struct RuntimeNormalizedRunCommand {
    /// runtime 输入来源参数。
    #[command(flatten)]
    input: InputArgs,

    /// 最多推进的回合数。
    #[arg(long = "max-rounds", default_value_t = 20_000, value_parser = parse_positive_usize, value_name = "N")]
    max_rounds: usize,
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
    /// 两队之间用换行分隔，队内默认用 `+` 分隔；传入 `--double-plus` 时队内改用 `++` 分隔。
    ///
    /// 示例:
    ///   tswn-cli bench win-rate -r "1@a+2@a\n3@b+4@b" -n 10000
    ///   tswn-cli bench win-rate -f teams.txt --double-plus --keep-rq --perf
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
    /// 靶子文件和选手文件每行一组，组内默认用 + 分隔，跳过空行；靶子侧的组内分隔
    /// 可用 `--target-list-double-plus` 改成 `++`，避免拆开名字里的 `+diy[...]` /
    /// `+ol:...`。
    /// `--out-file` 默认输出 `winrate<space>name`；`--log` 切到 JSONL，`--pure` 切到仅名字。
    /// `--min-screen` 控制终端显示阈值；`--min-file` 控制文件写入阈值（均为 0~100）。
    /// `--show-matchups` 以块状格式追加每个靶子的明细胜率（`平均胜率 名字` 下逐行
    /// 缩进 `胜率 靶子组`）；`--sort` 让输出文件按分数降序重排（`--pure` 不排）；
    /// `--clean-label` 把屏幕与文件标签里的 `+ol:` / `+diy[` 覆盖后缀剥掉。
    ///
    /// 示例:
    ///   tswn-cli bench batch-rate -l targets.txt -p players.txt -n 10000 -t 8
    ///   tswn-cli bench cqp -l targets.txt -p players.txt -n 10000 -t 8
    ///   tswn-cli bench cqp -l targets.txt -p players.txt --min-screen 60.5
    ///   tswn-cli bench batch-rate -l targets.txt -p players.txt -o result.txt --min-file 65
    ///   tswn-cli bench batch-rate -l targets.txt -p players.txt -o result.jsonl --log
    ///   tswn-cli bench cqp -l targets.txt -p players.txt --show-matchups --sort --clean-label
    #[command(
        name = "batch-rate",
        visible_alias = "cqp",
        verbatim_doc_comment
    )]
    BatchRate(BenchBatchRateCommand),
    /// 为每个选手和 teammate-list 中的每个队友组成二人组，计算各组合 batch rate 后取最高 head 个求和。
    ///
    /// player-list 和 teammate-list 均为每行一个组合；player-list 组内默认用 `+` 分隔，
    /// 可用 `--player-list-double-plus` 改成 `++`；teammate-list 组内默认用 `++`，
    /// 可用 `--teammate-list-single-plus` 改成 `+`。
    ///
    /// `--teammate-factored` 把 teammate-list 按带权 TOML 解析（与 `--target-factored`
    /// 的靶子文件同格式：`[[targets]]` 的 `factor` 与 `players`）：每个队友组合先按
    /// 靶子权重得到平均胜率，再乘该队友组的 `factor`，然后按 head 取高分求和——
    /// 队友权重影响排名与最终分数，不只是展示。
    ///
    /// `--detail` 控制 cqp 详情：`none`（默认）不输出；`every` 输出所有不低于
    /// `--detail-min` 的队友组合；`top` 输出最终取分的前 head 个。详情行格式为
    /// `最终分数 名字` 下逐行缩进 `cqp 队友组合`。`--detail-min` 只在 `every` 下生效。
    /// `--sort` 让输出文件按最终分数降序重排（`--pure` 不排）；`--clean-label` 把屏幕与
    /// 文件标签里的 `+ol:` / `+diy[` 覆盖后缀剥掉。
    ///
    /// 示例:
    ///   tswn-cli bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 3 -n 10000
    ///   tswn-cli bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 5 -o result.txt
    ///   tswn-cli bench pair -l targets.toml -p players.txt --teammate-list teammates.toml --target-factored --teammate-factored --head 4
    ///   tswn-cli bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 3 --detail top
    ///   tswn-cli bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 3 --detail every --detail-min 60
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
    /// 使用提供的两队文本作为输入，支持 `\n` 换行；支持 `-r/--raw`。
    #[arg(
        short = 'r',
        long,
        required_unless_present = "file",
        conflicts_with = "file",
        value_name = "STRING"
    )]
    raw: Option<String>,

    /// 从文件读取两队文本；支持 `-f/--file`。
    #[arg(
        short = 'f',
        long,
        required_unless_present = "raw",
        conflicts_with = "raw",
        value_name = "FILE"
    )]
    file: Option<PathBuf>,

    /// 胜率测试公共参数。
    #[command(flatten)]
    options: BenchOptions,

    /// 保持 rq=4，不模拟 JS win_rate 对 rq 的污染。
    #[arg(long)]
    keep_rq: bool,

    /// 队内使用 `++` 分隔，避免拆开名字里的 `+diy[...]` / `+ol:...`。
    #[arg(long = "double-plus")]
    double_plus: bool,
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
    /// 靶子列表文件；默认每行一组、组内用 + 分隔。使用 --target-factored 时读取带权 TOML。
    #[arg(short = 'l', long = "target-list", value_name = "FILE")]
    target_list: PathBuf,

    /// 选手列表文件，每行一组，组内用 + 分隔，跳过空行；支持 `-p/--player-list`。
    #[arg(short = 'p', long = "player-list", value_name = "FILE")]
    player_list: PathBuf,

    /// 使用 ++ 分隔 player-list 中的组内成员，避免拆开名字里的 +diy[...] / +ol:...。
    #[arg(long = "player-list-double-plus")]
    player_list_double_plus: bool,

    /// 靶子列表也使用 `++` 分隔组内成员（默认 `+`）。
    #[arg(long = "target-list-double-plus")]
    target_list_double_plus: bool,

    /// 将 target-list 按带权 TOML 解析，并按 factor 计算加权平均值。
    #[arg(long = "target-factored", alias = "weighted-targets")]
    target_factored: bool,

    /// 批量胜率测试的公共基准测试参数。
    #[command(flatten)]
    options: BenchOptions,

    /// 显示逐个靶子的明细胜率。
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// 以块状格式逐个靶子输出明细胜率（对齐 openbox 的“每组胜率”）。
    #[arg(long = "show-matchups")]
    show_matchups: bool,

    /// 输出文件按平均胜率降序重排（对齐 openbox；`--pure` 模式下不排序）。
    #[arg(long = "sort")]
    sort: bool,

    /// 屏幕与文件标签剥掉 `+ol:` / `+diy[` 覆盖后缀（对齐 openbox）。
    #[arg(long = "clean-label")]
    clean_label: bool,

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
    /// 靶子列表文件；默认每行一组、组内用 + 分隔。使用 --target-factored 时读取带权 TOML。
    #[arg(short = 'l', long = "target-list", value_name = "FILE")]
    target_list: PathBuf,

    /// 选手列表文件，每行一个组合，跳过空行；支持 `-p/--player-list`。
    #[arg(short = 'p', long = "player-list", value_name = "FILE")]
    player_list: PathBuf,

    /// 使用 `++` 分隔 player-list 每行中的成员；默认使用单个 `+`。
    #[arg(long = "player-list-double-plus")]
    player_list_double_plus: bool,

    /// 队友列表文件，每行一个组合，跳过空行。
    #[arg(long = "teammate-list", value_name = "FILE")]
    teammate_list: PathBuf,

    /// 使用单个 `+` 分隔 teammate-list 每行中的成员；默认使用 `++`。
    #[arg(
        long = "teammate-list-single-plus",
        conflicts_with = "teammate_list_double_plus"
    )]
    teammate_list_single_plus: bool,

    /// 显式使用 `++` 分隔 teammate-list 每行中的成员（默认行为）。
    #[arg(
        long = "teammate-list-double-plus",
        hide = true,
        conflicts_with = "teammate_list_single_plus"
    )]
    teammate_list_double_plus: bool,

    /// 将 target-list 按带权 TOML 解析，并按 factor 计算加权平均值。
    #[arg(long = "target-factored", alias = "weighted-targets")]
    target_factored: bool,

    /// 将 teammate-list 按带权 TOML 解析，队友组合的平均胜率先乘 factor 再按 head 取高分求和。
    #[arg(long = "teammate-factored", alias = "weighted-teammates")]
    teammate_factored: bool,

    /// 每名选手取最高的 N 个二人组 batch rate 求和。
    #[arg(long = "head", value_parser = parse_positive_usize, value_name = "N")]
    head: usize,

    /// `pair` 测试的公共基准测试参数。
    #[command(flatten)]
    options: BenchOptions,

    /// 显示逐个队友和靶子的明细胜率。
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// cqp 详情模式：none 不输出；every 输出所有不低于 --detail-min 的队友组合；top 输出前 head 个。
    #[arg(
        long = "detail",
        value_enum,
        value_name = "MODE",
        default_value = "none",
        verbatim_doc_comment
    )]
    detail: PairDetailArg,

    /// `--detail every` 的队友组合 cqp 阈值；其他 detail 模式下忽略。
    #[arg(long = "detail-min", value_parser = parse_non_negative_f64, value_name = "N")]
    detail_min: Option<f64>,

    /// 输出文件按最终分数降序重排（对齐 openbox；`--pure` 模式下不排序）。
    #[arg(long = "sort")]
    sort: bool,

    /// 屏幕与文件标签剥掉 `+ol:` / `+diy[` 覆盖后缀（对齐 openbox）。
    #[arg(long = "clean-label")]
    clean_label: bool,

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

    /// 保持 rq=4，而不使用 win-rate/profile rq。
    #[arg(long)]
    keep_rq: bool,

    /// 要保留的分数小数位数（默认：0）。
    #[arg(long = "precision", default_value_t = 0, value_parser = parse_wr_precision, value_name = "N")]
    precision: usize,

    /// 单个评分项的输出配置，可重复传入；语法 `NAME[:MIN_SCREEN[:FILE[:MIN_FILE]]]`。
    ///
    /// NAME 取 pp/pd/qp/qd/sum；MIN_SCREEN 是屏幕输出阈值（分），FILE 是输出文件，
    /// MIN_FILE 是文件写入阈值（分）。空段表示跳过，如 `pp::pp.txt` 只写文件不设阈值。
    /// 不传时默认 pp/pd/qp/qd/sum 五项全部输出到屏幕、无阈值、不写文件。
    /// FILE 路径不要包含 `:`（Windows 盘符前缀除外）。
    #[arg(long = "metric", value_name = "SPEC", value_parser = parse_metric_spec, verbatim_doc_comment)]
    metrics: Vec<NamerPfMetricSpec>,

    /// 只写文件、不在屏幕输出（需要至少一个 `--metric` 配置了 FILE）。
    #[arg(long = "no-screen")]
    no_screen: bool,

    /// 技能榜阈值配置（TOML，形如 `[sklfire]`/`[lessskl]` 的 pp/qp/qd/all 阈值表）。
    /// 指定后强制计算全部四项评分，并按阈值输出 `技能名指标 分数 名字` 行。
    #[arg(long = "skill-board", value_name = "FILE")]
    skill_board: Option<PathBuf>,

    /// 技能榜结果输出文件；未指定时只输出到屏幕。需要同时指定 `--skill-board`。
    #[arg(long = "skill-board-out", value_name = "FILE")]
    skill_board_out: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PairDetailArg {
    None,
    Every,
    Top,
}

impl From<PairDetailArg> for PairDetailMode {
    fn from(value: PairDetailArg) -> Self {
        match value {
            PairDetailArg::None => Self::None,
            PairDetailArg::Every => Self::Every,
            PairDetailArg::Top => Self::Top,
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

    /// 关闭单号模式的原始信息详情输出（默认开启）。
    #[arg(long = "no-details")]
    no_details: bool,
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
                jsonl: cmd.jsonl,
                max_rounds: cmd.max_rounds,
            },
            CliCommand::Runtime(RuntimeCommand { command }) => match command {
                RuntimeSubcommand::Diff(cmd) => ParsedCommand::RuntimeDiff {
                    raw: cmd.input.read_or_stdin()?,
                },
                RuntimeSubcommand::NormalizedRun(cmd) => ParsedCommand::RuntimeNormalizedRun {
                    raw: cmd.input.read_or_stdin()?,
                    max_rounds: cmd.max_rounds,
                },
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
                BenchSubcommand::WinRate(cmd) => {
                    let input = match (&cmd.raw, &cmd.file) {
                        (Some(raw), None) => decode_raw(raw),
                        (None, Some(path)) => read_file(path)?,
                        _ => return Err(cli_error("bench win-rate 只能使用 --raw 或 --file 其中一种输入")),
                    };
                    let (team1, team2) = parse_win_rate_teams(&input, cmd.double_plus)?;
                    ParsedCommand::BenchWinRate {
                        team1,
                        team2,
                        n: cmd.options.count.max(1),
                        mode: cmd.options.mode(),
                        threads: cmd.options.thread,
                        perf: cmd.options.perf,
                        keep_rq: cmd.keep_rq,
                        buckets_step: cmd.options.buckets_step,
                    }
                }
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
                    let (target_groups, target_factors) = if cmd.target_factored {
                        parse_factored_target_groups(&target_content)?
                    } else {
                        let groups = parse_plus_separated_groups(&target_content, cmd.target_list_double_plus);
                        let factors = vec![1.0; groups.len()];
                        (groups, factors)
                    };
                    let player_content = read_file(&cmd.player_list)?;
                    let (player_groups, player_labels) =
                        parse_player_groups_with_labels(&player_content, cmd.player_list_double_plus);
                    ParsedCommand::BenchBatchRate {
                        target_groups,
                        target_factors,
                        target_factored: cmd.target_factored,
                        player_groups,
                        player_labels,
                        n: cmd.options.count.max(1),
                        mode: cmd.options.mode(),
                        threads: cmd.options.thread,
                        perf: cmd.options.perf,
                        verbose: cmd.verbose,
                        show_matchups: cmd.show_matchups,
                        out_file: cmd.out_file,
                        force: cmd.force,
                        keep_rq: cmd.keep_rq,
                        log: cmd.log,
                        pure: cmd.pure,
                        min_screen: cmd.min_screen,
                        min_file: cmd.min_file,
                        wr_precision: cmd.wr_precision,
                        sort: cmd.sort,
                        clean_label: cmd.clean_label,
                    }
                }
                BenchSubcommand::Pair(cmd) => {
                    let target_content = read_file(&cmd.target_list)?;
                    let (target_groups, target_factors) = if cmd.target_factored {
                        parse_factored_target_groups(&target_content)?
                    } else {
                        let groups = parse_plus_separated_groups(&target_content, false);
                        let factors = vec![1.0; groups.len()];
                        (groups, factors)
                    };
                    let player_content = read_file(&cmd.player_list)?;
                    let teammate_content = read_file(&cmd.teammate_list)?;
                    let (players, player_labels) = parse_player_groups_with_labels(&player_content, cmd.player_list_double_plus);
                    // 带权队友 TOML 复用靶子格式：先按 `\n` 拼组，标签再按 `+` 拼回一行，
                    // 与 openbox `parse_pair_teammate_groups` 的处理一致。
                    let (teammates, teammate_labels, teammate_factors) = if cmd.teammate_factored {
                        let (groups, factors) = parse_factored_target_groups(&teammate_content)?;
                        let labels = groups.iter().map(|group| group.lines().collect::<Vec<_>>().join("+")).collect();
                        (groups, labels, factors)
                    } else {
                        let (groups, labels) = parse_player_groups_with_labels(
                            &teammate_content,
                            !cmd.teammate_list_single_plus || cmd.teammate_list_double_plus,
                        );
                        let factors = vec![1.0; groups.len()];
                        (groups, labels, factors)
                    };
                    ParsedCommand::BenchPair {
                        target_groups,
                        target_factors,
                        target_factored: cmd.target_factored,
                        teammate_factored: cmd.teammate_factored,
                        teammate_factors,
                        players,
                        player_labels,
                        teammates,
                        teammate_labels,
                        head: cmd.head,
                        n: cmd.options.count.max(1),
                        mode: cmd.options.mode(),
                        threads: cmd.options.thread,
                        perf: cmd.options.perf,
                        verbose: cmd.verbose,
                        detail: cmd.detail.into(),
                        detail_min: cmd.detail_min,
                        out_file: cmd.out_file,
                        force: cmd.force,
                        keep_rq: cmd.keep_rq,
                        log: cmd.log,
                        pure: cmd.pure,
                        min_screen: cmd.min_screen,
                        min_file: cmd.min_file,
                        wr_precision: cmd.wr_precision,
                        sort: cmd.sort,
                        clean_label: cmd.clean_label,
                    }
                }
            },
            CliCommand::NamerPf(cmd) => {
                // 默认五项全上屏；显式传入时按 GUI 的固定顺序归一化，并拒绝重复项与重复输出文件。
                let mut metrics = cmd.metrics;
                if metrics.is_empty() {
                    metrics = NamerPfMetric::ALL
                        .into_iter()
                        .map(|metric| NamerPfMetricSpec {
                            metric,
                            min_screen: None,
                            output_file: None,
                            min_file: None,
                        })
                        .collect();
                } else {
                    let mut seen = HashSet::new();
                    for spec in &metrics {
                        if !seen.insert(spec.metric) {
                            return Err(cli_error(format!("评分项重复: {}", spec.metric.label())));
                        }
                    }
                    metrics.sort_by_key(|spec| {
                        NamerPfMetric::ALL.iter().position(|metric| *metric == spec.metric).unwrap_or(usize::MAX)
                    });
                }
                let mut output_files = HashSet::new();
                for spec in &metrics {
                    if let Some(path) = spec.output_file.as_ref()
                        && !output_files.insert(path.clone())
                    {
                        return Err(cli_error(format!("输出文件被多个评分项引用: {}", path.display())));
                    }
                }
                if cmd.skill_board_out.is_some() && cmd.skill_board.is_none() {
                    return Err(cli_error("--skill-board-out 需要同时指定 --skill-board"));
                }
                if cmd.no_screen && metrics.iter().all(|spec| spec.output_file.is_none()) {
                    return Err(cli_error("--no-screen 需要至少一个 --metric 配置 FILE 段"));
                }
                ParsedCommand::NamerPf {
                    raw: cmd.input.read_or_stdin()?,
                    n: cmd.count.max(1),
                    threads: cmd.thread,
                    keep_rq: cmd.keep_rq,
                    precision: cmd.precision,
                    metrics,
                    no_screen: cmd.no_screen,
                    skill_board_config: cmd.skill_board,
                    skill_board_output: cmd.skill_board_out,
                }
            }
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
                    details: !cmd.no_details,
                }
            }
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
#[path = "cli_tests.rs"]
mod cli_tests;

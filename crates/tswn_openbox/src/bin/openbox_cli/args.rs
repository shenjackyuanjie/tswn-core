//! `openbox-cli` 的参数树与预设解析。
//!
//! 这个入口与 `tswn_openbox` GUI 共用同一套后端（`tswn_openbox::backend`）与预设
//! （`tswn_openbox::presets`），因此输出与 GUI 面板逐字节一致，包括配置文件约定：
//! 靶子/队友预设读 `./setting/settings.toml`（缺失时自动释放内嵌默认资源），
//! 技能榜阈值默认读 `./setting/score_now.toml`，也可用 `--skill-board FILE` 显式指定。
//!
//! 与 GUI 的刻意差异（均为 CLI 合理性考虑）：
//! - 高亮（GUI 的红色超强行）在 CLI 里降级为普通行，保证管道友好；
//! - 停止按钮语义不适用，Ctrl+C 直接终止进程；
//! - `--show-matchups` / `--detail` 默认跟随 GUI 勾选状态（开），可用负向开关关闭。

use std::collections::HashSet;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use tswn_openbox::backend::{NamerPfMetric, NamerPfMetricOptions, OutputMode, PairDetailMode};
use tswn_openbox::presets::{TargetPresetState, TeammatePresetState};

use super::input::{
    cli_error, decode_raw, parse_metric_spec, parse_non_negative_f64, parse_percent_0_100, parse_thread_count,
    parse_wr_precision, read_file,
};
use super::plan::{MetricSpec, SkillBoardPlan};

#[derive(Debug, Parser)]
#[command(
    name = "openbox-cli",
    about = "tswn openbox 的无头 CLI：与 GUI 面板同源、同输出、同配置",
    long_about = None,
    version = env!("CARGO_PKG_VERSION"),
    disable_help_subcommand = true,
    subcommand_required = true,
    arg_required_else_help = true
)]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Command,
}

#[derive(Debug, Subcommand)]
pub(super) enum Command {
    /// 将名字导出为 DIY / OL 覆盖格式（对齐 GUI 的 to-diy 面板）。
    ///
    /// 单号模式在未指定 `-o` 且名字不含 `+` 时附加输出原始信息详情
    /// （名字/队伍/八围/技能/name_factor），`--no-details` 可关闭；文件批量
    /// 模式不输出详情。默认输出 `+ol`；`--old` 切旧版 `+diy`；`--minions`
    /// 附带幻影/使魔/丧尸模板（与 `--old` 互斥）。
    ///
    /// 示例:
    ///   openbox-cli to-diy -r "mario@team+fire"
    ///   openbox-cli to-diy -f names.txt --minions
    ///   openbox-cli to-diy -r "地狱之轮 #mW88BamWo@Shabby_fish" --no-details
    #[command(name = "to-diy", verbatim_doc_comment)]
    ToDiy(ToDiyArgs),

    /// 五项评分（pp/pd/qp/qd/sum）与技能榜（对齐 GUI 的 namer-pf 面板）。
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
    /// 空段表示跳过，如 `pp::pp.txt` 只写文件不设阈值；指标固定按
    /// pp/pd/qp/qd/sum 顺序输出；同一指标或同一输出文件只能出现一次。
    /// FILE 路径不要包含 `:`（Windows 盘符前缀除外）。
    ///
    /// `--skill-board [FILE]` 开启技能榜：不带 FILE 时按 GUI 惯例读
    /// `./setting/score_now.toml`，带 FILE 时用显式阈值文件。阈值 TOML 形如
    /// `[sklfire]` / `[sklice]` 小节写 pp/qp/qd/all 阈值，`[lessskl]` 写白板号
    /// 阈值。程序把每个名字导出 `+diy`、取熟练度最高的技能，分数超过阈值时输出
    /// `技能名指标 分数 名字`（如 `冰冻qp 6647 mario`）；全部技能等级小于 30 时
    /// 改按 `[lessskl]` 阈值；`全能` 行还需同时满足 pp>=8000、pd>=9000、qp>=6000、
    /// qd>=7000。开启技能榜会强制计算全部四项评分。
    /// `--skill-board-out FILE` 单独写技能榜结果；`--no-screen` 只写文件。
    ///
    /// 示例:
    ///   openbox-cli namer-pf -r "mario"
    ///   openbox-cli namer-pf -f names.txt --metric sum --metric pp:8000
    ///   openbox-cli namer-pf -f names.txt --metric pp:8000:pp.txt:7500 --no-screen
    ///   openbox-cli namer-pf -f names.txt --skill-board --skill-board-out board.txt
    #[command(name = "namer-pf", verbatim_doc_comment)]
    NamerPf(NamerPfArgs),

    /// 批量计算选手列表对靶子列表的平均胜率（GUI 的 cqd/cqp 面板）。
    ///
    /// 靶子来自 `--targets FILE`（手动）或 `--target-preset ID`（settings.toml
    /// 预设，默认第一个；带权预设自动按 factor 加权，预设的 diy 字段决定是否用
    /// `++` 分隔）。选手来自 `--players FILE` 或 `-r`。
    /// `--show-matchups`（默认开，GUI 同）以块状格式逐个靶子输出明细胜率；
    /// `--no-show-matchups` 关闭。`-o FILE` 输出 `分数 名字`；`--log` 切 JSONL，
    /// `--pure` 切仅名字（写完按分数降序重排，pure 除外，与 GUI 一致）。
    /// `--min-screen` / `--min-file` 为 0~100 阈值；`--clean-label` 默认开启
    /// （GUI 行为：剥掉标签里的 `+ol:` / `+diy[` 后缀）。
    ///
    /// 示例:
    ///   openbox-cli cqd --target-preset 2 -p players.txt -n 10000
    ///   openbox-cli cqd -l targets.txt -p players.txt --min-screen 60.5
    ///   openbox-cli cqd -l targets.txt -p players.txt -o result.txt --log
    ///   openbox-cli cqd -l targets.txt -p players.txt --no-show-matchups
    #[command(name = "cqd", visible_alias = "cqp", verbatim_doc_comment)]
    Cqd(CqdArgs),

    /// 二人组评估：选手 × 队友组合取最高 head 个 batch rate 求和（GUI 的 pair 面板）。
    ///
    /// 靶子默认用 settings.toml 中 `id = 2` 的预设（不存在则第一个），也可
    /// `--targets FILE` 手动指定。队友默认用第一个队友预设（其 head 与
    /// factor_enabled 一并生效），也可 `--teammates FILE` 手动指定（此时用
    /// `--head`，默认 3）。`--teammate-preset NAME` 按名字选择队友预设。
    /// 选手每行默认 `+` 分隔（`--player-double-plus` 改 `++`）；队友每行默认
    /// `++` 分隔（`--teammate-single-plus` 改 `+`）。
    ///
    /// 带权队友 TOML（`--teammate-factored` 由队友预设的 factor_enabled 自动
    /// 触发，或队友文件本身是 TOML 时）的计算顺序：先按靶子权重得到平均胜率，
    /// 再乘队友组 `factor`，然后按乘权分数降序取前 head 个求和。
    ///
    /// `--detail none|every|top`（默认 every，GUI 同）控制 cqp 详情：
    /// every 输出所有不低于 `--detail-min` 的队友组合，top 输出最终取分的
    /// 前 head 个。块状格式为 `最终分数 名字` + 缩进 `  cqp 队友`。
    ///
    /// 示例:
    ///   openbox-cli pair -p players.txt --teammate-preset 刺评 --head 4
    ///   openbox-cli pair -l targets.txt -p players.txt --teammate-list mates.toml --teammate-factored
    ///   openbox-cli pair -p players.txt --teammates mates.txt --head 3 --detail top
    #[command(name = "pair", verbatim_doc_comment)]
    Pair(PairArgs),
}

/// 四个工具共用的基准参数（对齐 GUI 的公共设置区）。
#[derive(Debug, Args)]
pub(super) struct BenchOptions {
    /// 运行场数（默认 10000；GUI 的 1%/10%/100% 分别对应 100/1000/10000）。
    #[arg(short = 'n', long = "count", value_name = "N")]
    count: Option<usize>,

    /// 指定 benchmark 线程数；默认自动（约系统线程 × 1.5）。
    #[arg(short = 't', long = "thread", value_parser = parse_thread_count, value_name = "N")]
    thread: Option<usize>,

    /// 强制单线程运行；与 `--thread` 互斥。
    #[arg(
        short = 's',
        long = "single-thread",
        conflicts_with = "thread"
    )]
    single_thread: bool,

    /// 保持 rq=4，不模拟 JS win_rate 对 rq 的污染（GUI 的“不低估短号”；
    /// namer-pf 默认关，cqd/pair 默认开，与 GUI 勾选状态一致）。
    /// 支持 `--keep-rq` / `--keep-rq=false` / `--keep-rq true` 三种写法。
    #[arg(long, value_name = "BOOL", num_args = 0..=1, default_missing_value = "true")]
    keep_rq: Option<bool>,
}

#[derive(Debug, Args)]
pub(super) struct ToDiyArgs {
    /// 单个玩家名字（namerena raw 格式，支持 @ 队伍名和 + 武器名）。
    #[arg(
        short = 'r',
        long = "raw",
        value_name = "NAME",
        conflicts_with = "file"
    )]
    raw: Option<String>,

    /// 从文件按行读取多个玩家名字；空行跳过，输出按行对应。
    #[arg(short = 'f', long = "file", value_name = "FILE")]
    file: Option<PathBuf>,

    /// 将结果写入指定文件；未指定时输出到标准输出。
    #[arg(short = 'o', long = "out-file", value_name = "FILE")]
    out_file: Option<PathBuf>,

    /// 输出旧版 `+diy` 形式（默认 `+ol`）。
    #[arg(long = "old")]
    old: bool,

    /// 在 `+ol` 中附带幻影/使魔/丧尸模板；与 `--old` 互斥。
    #[arg(long = "minions", conflicts_with = "old")]
    minions: bool,

    /// 关闭单号模式的原始信息详情（默认开启，GUI 的“单名详情”复选框）。
    #[arg(long = "no-details")]
    no_details: bool,
}

#[derive(Debug, Args)]
pub(super) struct NamerPfArgs {
    /// 每行一个名字组、组内 `+` 分隔的输入文本（支持 `\n` 换行转义）。
    #[arg(
        short = 'r',
        long = "raw",
        value_name = "STRING",
        conflicts_with = "file"
    )]
    raw: Option<String>,

    /// 从文件读取输入。
    #[arg(
        short = 'f',
        long = "file",
        value_name = "FILE",
        conflicts_with = "raw"
    )]
    file: Option<PathBuf>,

    #[command(flatten)]
    options: BenchOptions,

    /// 分数保留小数位数（默认 0）。
    #[arg(long = "precision", default_value_t = 0, value_parser = parse_wr_precision, value_name = "N")]
    precision: usize,

    /// 单个评分项配置，可重复；语法 `NAME[:MIN_SCREEN[:FILE[:MIN_FILE]]]`。
    #[arg(long = "metric", value_name = "SPEC", value_parser = parse_metric_spec, verbatim_doc_comment)]
    metrics: Vec<MetricSpec>,

    /// 只写文件、不在屏幕输出（需要至少一个 `--metric` 配置 FILE 或技能榜输出文件）。
    #[arg(long = "no-screen")]
    no_screen: bool,

    /// 开启技能榜；可带阈值 TOML 路径，不带则读 `./setting/score_now.toml`。
    #[arg(long = "skill-board", value_name = "FILE", num_args = 0..=1, default_missing_value = "")]
    skill_board: Option<String>,

    /// 技能榜结果输出文件；未指定时只输出到屏幕。需要同时开启 `--skill-board`。
    #[arg(long = "skill-board-out", value_name = "FILE")]
    skill_board_out: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(super) struct CqdArgs {
    /// 靶子列表文件；每行一组、组内默认 `+` 分隔。使用 `--target-factored` 时读带权 TOML。
    #[arg(
        short = 'l',
        long = "targets",
        value_name = "FILE",
        conflicts_with = "target_preset"
    )]
    targets: Option<PathBuf>,

    /// 选择 settings.toml 中的靶子预设 id；与 `--targets` 互斥。
    #[arg(
        long = "target-preset",
        value_name = "ID",
        conflicts_with = "targets"
    )]
    target_preset: Option<u64>,

    /// 选手列表文件；每行一组，组内默认 `+` 分隔。
    #[arg(
        short = 'p',
        long = "players",
        value_name = "FILE",
        conflicts_with = "raw"
    )]
    players: Option<PathBuf>,

    /// 选手输入文本（支持 `\n` 换行转义），每行一个组合。
    #[arg(
        short = 'r',
        long = "raw",
        value_name = "STRING",
        conflicts_with = "players"
    )]
    raw: Option<String>,

    /// 选手行内使用 `++` 分隔（默认 `+`）。
    #[arg(long = "player-double-plus")]
    player_double_plus: bool,

    /// 手动靶子行内使用 `++` 分隔（默认 `+`；预设靶子的分隔由预设 diy 字段决定）。
    #[arg(long = "double-plus", requires = "targets")]
    double_plus: bool,

    #[command(flatten)]
    options: BenchOptions,

    /// 胜率小数位数（默认 3）。
    #[arg(long = "wr-precision", default_value_t = 3, value_parser = parse_wr_precision, value_name = "N")]
    wr_precision: usize,

    /// 块状输出逐个靶子的明细胜率（默认开启；`--no-show-matchups` 关闭）。
    #[arg(long = "show-matchups", overrides_with = "no_show_matchups")]
    show_matchups: bool,

    /// 关闭逐个靶子的块状明细。
    #[arg(long = "no-show-matchups")]
    no_show_matchups: bool,

    /// 仅在终端显示平均胜率不低于此值的选手（0~100）。
    #[arg(long = "min-screen", value_parser = parse_percent_0_100, value_name = "N")]
    min_screen: Option<f64>,

    /// 只写入平均胜率不低于此值的选手（0~100）。
    #[arg(long = "min-file", requires = "out_file", value_parser = parse_percent_0_100, value_name = "N")]
    min_file: Option<f64>,

    /// 输出格式：默认 `分数 名字`；`--log` 切 JSONL；`--pure` 切仅名字。
    #[arg(long = "log", conflicts_with = "pure")]
    log: bool,

    /// 每行只输出 `名字`。
    #[arg(long = "pure", conflicts_with = "log")]
    pure: bool,

    /// 结果输出文件；写完按分数降序重排（pure 除外）。
    #[arg(short = 'o', long = "out-file", value_name = "FILE")]
    out_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(super) struct PairArgs {
    /// 靶子列表文件；每行一组、组内 `+` 分隔；带权 TOML 时按 factor 加权。
    #[arg(
        short = 'l',
        long = "targets",
        value_name = "FILE",
        conflicts_with = "target_preset"
    )]
    targets: Option<PathBuf>,

    /// 选择 settings.toml 中的靶子预设 id（默认 2，不存在则第一个）。
    #[arg(
        long = "target-preset",
        value_name = "ID",
        conflicts_with = "targets"
    )]
    target_preset: Option<u64>,

    /// 选手列表文件；每行一个组合，组内默认 `+` 分隔。
    #[arg(
        short = 'p',
        long = "players",
        value_name = "FILE",
        conflicts_with = "raw"
    )]
    players: Option<PathBuf>,

    /// 选手输入文本（支持 `\n` 换行转义），每行一个组合。
    #[arg(
        short = 'r',
        long = "raw",
        value_name = "STRING",
        conflicts_with = "players"
    )]
    raw: Option<String>,

    /// 选手行内使用 `++` 分隔（默认 `+`）。
    #[arg(long = "player-double-plus")]
    player_double_plus: bool,

    /// 队友列表文件；每行一个组合；带权 TOML 时按 factor 加权（自动启用带权）。
    #[arg(
        long = "teammates",
        value_name = "FILE",
        conflicts_with = "teammate_preset"
    )]
    teammates: Option<PathBuf>,

    /// 选择 settings.toml 中的队友预设名字；默认第一个队友预设。
    #[arg(
        long = "teammate-preset",
        value_name = "NAME",
        conflicts_with = "teammates"
    )]
    teammate_preset: Option<String>,

    /// 每名选手取最高的 N 个二人组 batch rate 求和（手动队友时默认 3；
    /// 队友预设未指定时用预设自带的 head）。
    #[arg(long = "head", value_name = "N")]
    head: Option<usize>,

    /// 队友行内使用单个 `+` 分隔（默认 `++`，GUI 同）。
    #[arg(long = "teammate-single-plus")]
    teammate_single_plus: bool,

    #[command(flatten)]
    options: BenchOptions,

    /// 胜率小数位数（默认 3）。
    #[arg(long = "wr-precision", default_value_t = 3, value_parser = parse_wr_precision, value_name = "N")]
    wr_precision: usize,

    /// cqp 详情模式（默认 every，GUI 同）。
    #[arg(
        long = "detail",
        value_enum,
        value_name = "MODE",
        default_value = "every"
    )]
    detail: PairDetailArg,

    /// `--detail every` 的队友组合 cqp 阈值；其他模式下忽略。
    #[arg(long = "detail-min", value_parser = parse_non_negative_f64, value_name = "N")]
    detail_min: Option<f64>,

    /// 仅在终端显示最终分数不低于此值的选手。
    #[arg(long = "min-screen", value_parser = parse_non_negative_f64, value_name = "N")]
    min_screen: Option<f64>,

    /// 只写入最终分数不低于此值的选手。
    #[arg(long = "min-file", requires = "out_file", value_parser = parse_non_negative_f64, value_name = "N")]
    min_file: Option<f64>,

    /// 输出格式：默认 `分数 名字`；`--log` 切 JSONL；`--pure` 切仅名字。
    #[arg(long = "log", conflicts_with = "pure")]
    log: bool,

    /// 每行只输出 `名字`。
    #[arg(long = "pure", conflicts_with = "log")]
    pure: bool,

    /// 结果输出文件；写完按最终分数降序重排（pure 除外）。
    #[arg(short = 'o', long = "out-file", value_name = "FILE")]
    out_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum PairDetailArg {
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

/// 默认参数对齐 GUI 勾选状态。
const DEFAULT_COUNT: usize = 10000;
const DEFAULT_HEAD: usize = 3;
const NAMER_PF_DEFAULT_KEEP_RQ: bool = false;
const BENCH_DEFAULT_KEEP_RQ: bool = true;

impl BenchOptions {
    fn count(&self) -> usize { self.count.unwrap_or(DEFAULT_COUNT).max(1) }

    fn threads(&self) -> Option<usize> { if self.single_thread { Some(1) } else { self.thread } }

    fn keep_rq(&self, namer_pf: bool) -> bool {
        self.keep_rq.unwrap_or(if namer_pf {
            NAMER_PF_DEFAULT_KEEP_RQ
        } else {
            BENCH_DEFAULT_KEEP_RQ
        })
    }
}

/// 解析命令行并归一化为执行计划（含预设解析与文件读取）。
pub(super) fn parse() -> Result<Cli, clap::Error> {
    let cli = Cli::try_parse()?;
    Ok(cli)
}

/// 读取输入文本：`-r` 优先（还原 `\n`），其次 `-f` 文件，最后报错。
fn read_input(raw: Option<&String>, file: Option<&PathBuf>, what: &str) -> Result<String, clap::Error> {
    match (raw, file) {
        (Some(raw), None) => Ok(decode_raw(raw)),
        (None, Some(path)) => read_file(path),
        (None, None) => Err(cli_error(format!("{what} 需要 -r/--raw 或文件输入"))),
        (Some(_), Some(_)) => Err(cli_error(format!("{what} 只能使用一种输入来源"))),
    }
}

/// 解析 namer-pf 的指标配置：默认五项全上屏；显式时去重并按固定顺序排序。
fn resolve_metrics(specs: Vec<MetricSpec>, no_screen: bool) -> Result<Vec<NamerPfMetricOptions>, clap::Error> {
    let mut ordered = specs;
    if ordered.is_empty() {
        ordered = NamerPfMetric::ALL
            .into_iter()
            .map(|metric| MetricSpec {
                metric,
                min_screen: None,
                output_file: None,
                min_file: None,
            })
            .collect();
    } else {
        let mut seen = HashSet::new();
        for spec in &ordered {
            if !seen.insert(spec.metric) {
                return Err(cli_error(format!("评分项重复: {}", spec.metric_label())));
            }
        }
        ordered.sort_by_key(|spec| NamerPfMetric::ALL.iter().position(|metric| *metric == spec.metric).unwrap_or(usize::MAX));
    }
    let mut files = HashSet::new();
    let mut metrics = Vec::with_capacity(ordered.len());
    for spec in ordered {
        if let Some(path) = spec.output_file.as_ref()
            && !files.insert(path.clone())
        {
            return Err(cli_error(format!("输出文件被多个评分项引用: {}", path.display())));
        }
        metrics.push(NamerPfMetricOptions {
            metric: spec.metric,
            screen: !no_screen,
            min_screen: spec.min_screen,
            output_file: spec.output_file,
            min_file: spec.min_file,
            highlight_delta: None,
        });
    }
    Ok(metrics)
}

/// 解析技能榜开关：`--skill-board` 出现即开启；空串表示用默认路径。
fn resolve_skill_board(skill_board: Option<String>, skill_board_out: Option<PathBuf>) -> Result<SkillBoardPlan, clap::Error> {
    let Some(raw) = skill_board else {
        if skill_board_out.is_some() {
            return Err(cli_error("--skill-board-out 需要同时指定 --skill-board"));
        }
        return Ok(SkillBoardPlan::default());
    };
    let config = (!raw.is_empty()).then(|| PathBuf::from(raw));
    Ok(SkillBoardPlan {
        enabled: true,
        output_file: skill_board_out,
        config,
    })
}

/// 靶子来源：预设（含 factor/diy 语义）或手动文件。
#[derive(Debug)]
struct ResolvedTarget {
    text: String,
    factor_enabled: bool,
    double_plus: bool,
}

fn resolve_target(
    targets: Option<&PathBuf>,
    target_preset: Option<u64>,
    manual_double_plus: bool,
    preferred_preset_id: Option<u64>,
) -> Result<ResolvedTarget, clap::Error> {
    if let Some(path) = targets {
        let text = read_file(path)?;
        return Ok(ResolvedTarget {
            text,
            factor_enabled: false,
            double_plus: manual_double_plus,
        });
    }

    let state = TargetPresetState::load_with_preferred_id(preferred_preset_id);
    let preset = match target_preset {
        Some(id) => state.items.iter().find(|item| item.id == id).ok_or_else(|| {
            let ids = state.items.iter().map(|item| item.id.to_string()).collect::<Vec<_>>().join(", ");
            cli_error(format!("找不到靶子预设 id={id}；可用 id: {ids}"))
        })?,
        None => state.selected().ok_or_else(|| cli_error("settings.toml 中没有可用的靶子预设"))?,
    };
    let text = tswn_openbox::presets::load_target_preset_text(preset).map_err(cli_error)?;
    Ok(ResolvedTarget {
        text,
        factor_enabled: preset.factor_enabled,
        double_plus: preset.diy,
    })
}

/// 队友来源：预设（head/factor 语义）或手动文件。
#[derive(Debug)]
struct ResolvedTeammate {
    text: String,
    factor_enabled: bool,
    head: usize,
}

fn resolve_teammate(
    teammates: Option<&PathBuf>,
    teammate_preset: Option<&String>,
    head: Option<usize>,
) -> Result<ResolvedTeammate, clap::Error> {
    if let Some(path) = teammates {
        let text = read_file(path)?;
        return Ok(ResolvedTeammate {
            text,
            factor_enabled: false,
            head: head.unwrap_or(DEFAULT_HEAD).max(1),
        });
    }

    let state = TeammatePresetState::load();
    let preset = match teammate_preset {
        Some(name) => state.items.iter().find(|item| &item.name == name).ok_or_else(|| {
            let names = state.items.iter().map(|item| item.name.as_str()).collect::<Vec<_>>().join(", ");
            cli_error(format!("找不到队友预设 {name:?}；可用预设: {names}"))
        })?,
        None => state.selected().ok_or_else(|| cli_error("settings.toml 中没有可用的队友预设"))?,
    };
    let text = tswn_openbox::presets::load_teammate_preset_text(preset).map_err(cli_error)?;
    Ok(ResolvedTeammate {
        text,
        factor_enabled: preset.factor_enabled,
        head: head.unwrap_or(preset.head).max(1),
    })
}

/// namer-pf 的执行计划（不含 cancel，由 runner 注入）。
#[derive(Debug)]
pub(super) struct NamerPfPlan {
    pub raw: String,
    pub count: usize,
    pub threads: Option<usize>,
    pub keep_rq: bool,
    pub precision: usize,
    pub metrics: Vec<NamerPfMetricOptions>,
    pub no_screen: bool,
    pub skill_board: SkillBoardPlan,
}

/// cqd / pair 共用的输出计划。
#[derive(Debug)]
pub(super) struct OutputPlan {
    pub mode: OutputMode,
    pub out_file: Option<PathBuf>,
}

#[derive(Debug)]
pub(super) struct CqdPlan {
    pub target_text: String,
    pub target_factor_enabled: bool,
    pub target_double_plus: bool,
    pub player_text: String,
    pub player_double_plus: bool,
    pub show_matchups: bool,
    pub min_screen: Option<f64>,
    pub min_file: Option<f64>,
    pub wr_precision: usize,
    pub count: usize,
    pub threads: Option<usize>,
    pub keep_rq: bool,
    pub output: OutputPlan,
}

#[derive(Debug)]
pub(super) struct PairPlan {
    pub target_text: String,
    pub target_factor_enabled: bool,
    pub player_text: String,
    pub player_double_plus: bool,
    pub teammate_text: String,
    pub teammate_double_plus: bool,
    pub teammate_factor_enabled: bool,
    pub head: usize,
    pub detail: PairDetailMode,
    pub detail_min: Option<f64>,
    pub min_screen: Option<f64>,
    pub min_file: Option<f64>,
    pub wr_precision: usize,
    pub count: usize,
    pub threads: Option<usize>,
    pub keep_rq: bool,
    pub output: OutputPlan,
}

#[derive(Debug)]
pub(super) struct ToDiyPlan {
    pub raw: String,
    pub old: bool,
    pub minions: bool,
    pub details: bool,
    pub output_file: Option<PathBuf>,
}

fn output_mode(log: bool, pure: bool) -> OutputMode {
    if pure {
        OutputMode::Pure
    } else if log {
        OutputMode::Jsonl
    } else {
        OutputMode::Log
    }
}

impl Cli {
    /// 把 clap 结果归一化为执行计划（读文件、解析预设、校验参数组合）。
    pub(super) fn plan(self) -> Result<Job, clap::Error> {
        Ok(match self.command {
            Command::ToDiy(args) => {
                let raw = match (args.raw, args.file) {
                    (Some(raw), None) => decode_raw(&raw),
                    (None, Some(path)) => read_file(&path)?,
                    (None, None) => return Err(cli_error("to-diy 需要 -r/--raw 或 -f/--file")),
                    (Some(_), Some(_)) => return Err(cli_error("to-diy 只能使用一种输入来源")),
                };
                Job::ToDiy(ToDiyPlan {
                    raw,
                    old: args.old,
                    minions: args.minions,
                    details: !args.no_details,
                    output_file: args.out_file,
                })
            }
            Command::NamerPf(args) => {
                let raw = read_input(args.raw.as_ref(), args.file.as_ref(), "namer-pf")?;
                let skill_board = resolve_skill_board(args.skill_board, args.skill_board_out)?;
                let metrics = resolve_metrics(args.metrics, args.no_screen)?;
                // “至少一个输出”校验（GUI 同义）：全文件-only 时skill board也必须
                // 有落点，否则后端会因没有任何输出而直接失败。
                if args.no_screen
                    && metrics.iter().all(|metric| metric.output_file.is_none())
                    && !(skill_board.enabled && skill_board.output_file.is_some())
                {
                    return Err(cli_error(
                        "namer-pf: --no-screen 需要至少一个 --metric FILE 或 --skill-board-out",
                    ));
                }
                Job::NamerPf(NamerPfPlan {
                    raw,
                    count: args.options.count(),
                    threads: args.options.threads(),
                    keep_rq: args.options.keep_rq(true),
                    precision: args.precision,
                    metrics,
                    no_screen: args.no_screen,
                    skill_board,
                })
            }
            Command::Cqd(args) => {
                let target = resolve_target(args.targets.as_ref(), args.target_preset, args.double_plus, None)?;
                let player_text = read_input(args.raw.as_ref(), args.players.as_ref(), "cqd 选手")?;
                Job::Cqd(CqdPlan {
                    target_text: target.text,
                    target_factor_enabled: target.factor_enabled,
                    target_double_plus: target.double_plus,
                    player_text,
                    player_double_plus: args.player_double_plus,
                    show_matchups: !args.no_show_matchups,
                    min_screen: args.min_screen,
                    min_file: args.min_file,
                    wr_precision: args.wr_precision,
                    count: args.options.count(),
                    threads: args.options.threads(),
                    keep_rq: args.options.keep_rq(false),
                    output: OutputPlan {
                        mode: output_mode(args.log, args.pure),
                        out_file: args.out_file,
                    },
                })
            }
            Command::Pair(args) => {
                let target = resolve_target(args.targets.as_ref(), args.target_preset, false, Some(2))?;
                let teammate = resolve_teammate(args.teammates.as_ref(), args.teammate_preset.as_ref(), args.head)?;
                let player_text = read_input(args.raw.as_ref(), args.players.as_ref(), "pair 选手")?;
                Job::Pair(PairPlan {
                    target_text: target.text,
                    target_factor_enabled: target.factor_enabled,
                    player_text,
                    player_double_plus: args.player_double_plus,
                    teammate_text: teammate.text,
                    teammate_double_plus: !args.teammate_single_plus,
                    teammate_factor_enabled: teammate.factor_enabled,
                    head: teammate.head,
                    detail: args.detail.into(),
                    detail_min: args.detail_min,
                    min_screen: args.min_screen,
                    min_file: args.min_file,
                    wr_precision: args.wr_precision,
                    count: args.options.count(),
                    threads: args.options.threads(),
                    keep_rq: args.options.keep_rq(false),
                    output: OutputPlan {
                        mode: output_mode(args.log, args.pure),
                        out_file: args.out_file,
                    },
                })
            }
        })
    }
}

/// 归一化后的执行计划。
#[derive(Debug)]
pub(super) enum Job {
    ToDiy(ToDiyPlan),
    NamerPf(NamerPfPlan),
    Cqd(CqdPlan),
    Pair(PairPlan),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use tswn_openbox::backend::NamerPfMetric;

    fn plan_of(argv: &[&str]) -> Result<Job, clap::Error> {
        let cli = Cli::try_parse_from(std::iter::once("openbox-cli").chain(argv.iter().copied())).expect("参数应能解析");
        cli.plan()
    }

    #[test]
    fn to_diy_plan_defaults_details_on() {
        let job = plan_of(&["to-diy", "-r", "mario@team"]).unwrap();
        match job {
            Job::ToDiy(plan) => {
                assert_eq!(plan.raw, "mario@team");
                assert!(plan.details);
                assert!(!plan.old);
                assert!(!plan.minions);
                assert!(plan.output_file.is_none());
            }
            _ => panic!("unexpected job"),
        }
    }

    #[test]
    fn to_diy_rejects_old_with_minions() {
        let err = Cli::try_parse_from(["openbox-cli", "to-diy", "-r", "mario", "--old", "--minions"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn namer_pf_defaults_to_all_five_screen_metrics() {
        let job = plan_of(&["namer-pf", "-r", "mario"]).unwrap();
        match job {
            Job::NamerPf(plan) => {
                let labels = plan.metrics.iter().map(|m| m.metric).collect::<Vec<_>>();
                assert_eq!(labels, NamerPfMetric::ALL.to_vec());
                assert!(plan.metrics.iter().all(|m| m.screen && m.output_file.is_none()));
                assert!(!plan.no_screen);
                assert!(!plan.skill_board.enabled);
                assert_eq!(plan.count, DEFAULT_COUNT);
            }
            _ => panic!("unexpected job"),
        }
    }

    #[test]
    fn namer_pf_keep_rq_defaults_off() {
        let job = plan_of(&["namer-pf", "-r", "mario"]).unwrap();
        match job {
            Job::NamerPf(plan) => assert!(!plan.keep_rq),
            _ => panic!("unexpected job"),
        }
        let job = plan_of(&["namer-pf", "-r", "mario", "--keep-rq"]).unwrap();
        match job {
            Job::NamerPf(plan) => assert!(plan.keep_rq),
            _ => panic!("unexpected job"),
        }
        let job = plan_of(&["namer-pf", "-r", "mario", "--keep-rq=false"]).unwrap();
        match job {
            Job::NamerPf(plan) => assert!(!plan.keep_rq),
            _ => panic!("unexpected job"),
        }
    }

    #[test]
    fn namer_pf_metrics_normalize_order_and_files() {
        let job = plan_of(&["namer-pf", "-r", "mario", "--metric", "sum:30000", "--metric", "pp:8000"]).unwrap();
        match job {
            Job::NamerPf(plan) => {
                let labels = plan.metrics.iter().map(|m| m.metric).collect::<Vec<_>>();
                assert_eq!(labels, vec![NamerPfMetric::Pp, NamerPfMetric::Sum]);
                assert_eq!(plan.metrics[0].min_screen, Some(8000.0));
            }
            _ => panic!("unexpected job"),
        }
    }

    #[test]
    fn namer_pf_rejects_duplicate_metric_and_shared_file() {
        let err = plan_of(&["namer-pf", "-r", "mario", "--metric", "pp", "--metric", "pp"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
        let err = plan_of(&[
            "namer-pf",
            "-r",
            "mario",
            "--metric",
            "pp::same.txt",
            "--metric",
            "sum::same.txt",
        ])
        .unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn namer_pf_no_screen_requires_file_output() {
        let err = plan_of(&["namer-pf", "-r", "mario", "--no-screen"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
        // 有文件落点时允许。
        let job = plan_of(&["namer-pf", "-r", "mario", "--no-screen", "--metric", "pp::pp.txt"]).unwrap();
        match job {
            Job::NamerPf(plan) => {
                assert!(plan.no_screen);
                assert!(plan.metrics.iter().all(|m| !m.screen));
            }
            _ => panic!("unexpected job"),
        }
    }

    #[test]
    fn namer_pf_skill_board_flags() {
        // --skill-board-out 依赖 --skill-board。
        let err = plan_of(&["namer-pf", "-r", "mario", "--skill-board-out", "b.txt"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
        // 不带值 = 默认路径；带值 = 显式路径。
        let job = plan_of(&["namer-pf", "-r", "mario", "--skill-board"]).unwrap();
        match job {
            Job::NamerPf(plan) => {
                assert!(plan.skill_board.enabled);
                assert!(plan.skill_board.config.is_none());
                assert!(plan.skill_board.output_file.is_none());
            }
            _ => panic!("unexpected job"),
        }
        let job = plan_of(&["namer-pf", "-r", "mario", "--skill-board", "board.toml"]).unwrap();
        match job {
            Job::NamerPf(plan) => assert_eq!(plan.skill_board.config, Some(PathBuf::from("board.toml"))),
            _ => panic!("unexpected job"),
        }
    }

    /// 写临时文件并返回路径；文件带自增序号，并行测试互不覆盖。
    fn temp_file(name: &str, content: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("tswn_openbox_cli_{}_{}_{}", std::process::id(), name, seq));
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn cqd_plan_reads_manual_files_with_defaults() {
        let targets = temp_file("cqd_targets.txt", "1@a\n2@b");
        let players = temp_file("cqd_players.txt", "3@c");
        let job = plan_of(&["cqd", "-l", targets.to_str().unwrap(), "-p", players.to_str().unwrap()]).unwrap();
        match job {
            Job::Cqd(plan) => {
                assert_eq!(plan.target_text, "1@a\n2@b");
                assert_eq!(plan.player_text, "3@c");
                // GUI 默认勾选：每组胜率明细开、keep-rq 开、wr 精度 3。
                assert!(plan.show_matchups);
                assert!(plan.keep_rq);
                assert_eq!(plan.wr_precision, 3);
                assert!(!plan.target_factor_enabled);
                assert!(!plan.target_double_plus);
            }
            _ => panic!("unexpected job"),
        }
    }

    #[test]
    fn cqd_plan_can_disable_matchups_and_pick_output_mode() {
        let targets = temp_file("cqd_targets.txt", "1@a");
        let players = temp_file("cqd_players.txt", "3@c");
        let job = plan_of(&[
            "cqd",
            "-l",
            targets.to_str().unwrap(),
            "-p",
            players.to_str().unwrap(),
            "--no-show-matchups",
            "--pure",
            "-o",
            "out.txt",
            "--min-file",
            "60.5",
            "--double-plus",
        ])
        .unwrap();
        match job {
            Job::Cqd(plan) => {
                assert!(!plan.show_matchups);
                assert_eq!(plan.output.mode, OutputMode::Pure);
                assert!(plan.target_double_plus);
                assert_eq!(plan.min_file, Some(60.5));
            }
            _ => panic!("unexpected job"),
        }
    }

    #[test]
    fn pair_plan_manual_teammates_defaults() {
        let targets = temp_file("pair_targets.txt", "1@a");
        let players = temp_file("pair_players.txt", "3@c");
        let mates = temp_file("pair_mates.txt", "5@e");
        let job = plan_of(&[
            "pair",
            "-l",
            targets.to_str().unwrap(),
            "-p",
            players.to_str().unwrap(),
            "--teammates",
            mates.to_str().unwrap(),
        ])
        .unwrap();
        match job {
            Job::Pair(plan) => {
                // 手动队友：head 默认 3、队友默认 `++` 分隔（GUI 同）、detail 默认 every。
                assert_eq!(plan.head, 3);
                assert!(plan.teammate_double_plus);
                assert_eq!(plan.detail, PairDetailMode::Every);
                assert!(!plan.teammate_factor_enabled);
                assert!(!plan.player_double_plus);
            }
            _ => panic!("unexpected job"),
        }
    }

    #[test]
    fn pair_plan_single_thread_maps_to_thread_one() {
        let targets = temp_file("pair_targets.txt", "1@a");
        let players = temp_file("pair_players.txt", "3@c");
        let mates = temp_file("pair_mates.txt", "5@e");
        let job = plan_of(&[
            "pair",
            "-l",
            targets.to_str().unwrap(),
            "-p",
            players.to_str().unwrap(),
            "--teammates",
            mates.to_str().unwrap(),
            "-s",
        ])
        .unwrap();
        match job {
            Job::Pair(plan) => assert_eq!(plan.threads, Some(1)),
            _ => panic!("unexpected job"),
        }
    }

    #[test]
    fn cqd_target_preset_and_manual_target_conflict() {
        let err = Cli::try_parse_from(["openbox-cli", "cqd", "-l", "t.txt", "--target-preset", "1", "-p", "p.txt"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}

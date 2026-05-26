//! `tswn-cli` 在执行阶段使用的内部命令模型。
//!
//! 这一层故意不再暴露 `clap` 的结构细节。执行代码只关心“已经归一化之后的命令”，
//! 不关心这些值究竟来自 `--raw`、`--file` 还是 stdin，也不关心 `clap` 的字段命名。

use std::path::PathBuf;

/// benchmark 线程策略。
///
/// 这里保留一个非常薄的枚举，而不是直接把 `thread: Option<usize>` 暴露给执行层，
/// 是因为“显式单线程”和“未指定线程但允许并行”是两个不同的语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchThreadMode {
    /// 自动或显式并行运行 benchmark。
    Parallel,
    /// 强制使用单线程运行 benchmark。
    SingleThread,
}

/// 归一化后的 CLI 命令。
///
/// 这里的每个字段都已经过输入来源统一、基础校验和必要的文本转换：
/// - `raw` 字符串已经完成 `\n` 还原；
/// - 列表文件已经被读入并拆成执行阶段需要的结构；
/// - 线程模式已经从 `--single-thread` / `--thread` 整理成统一表示。
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
        /// 仅在输出到文件时生效：每行输出 `winrate<space>name`。
        log: bool,
        /// 仅在输出到文件时生效：每行只输出 `name`。
        pure: bool,
        /// 仅在终端显示平均胜率不低于此值的选手（0~100）。
        min_screen: Option<f64>,
        /// 仅在输出到文件时生效：只写入平均胜率不低于此值的选手（0~100）。
        min_file: Option<f64>,
        /// 胜率小数位数。
        wr_precision: usize,
    },
    BenchPair {
        /// 靶子组列表；每项都已从 `+` 分隔行转换成 `\n` 分隔的 namerena 组字符串。
        target_groups: Vec<String>,
        /// player-list 中的选手；每行一个名字。
        players: Vec<String>,
        /// teammate-list 中的队友；每行一个名字。
        teammates: Vec<String>,
        /// 每名选手取最高的 head 个二人组 batch rate 求和。
        head: usize,
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
        /// 批量结果输出文件；未指定时只输出到终端。
        out_file: Option<PathBuf>,
        /// 若输出文件已存在，是否直接覆盖而不再确认。
        force: bool,
        /// 是否保持 `rq=4`，不模拟 JS `win_rate` 对 `rq` 的污染。
        keep_rq: bool,
        /// 仅在输出到文件时生效：输出 JSONL。
        log: bool,
        /// 仅在输出到文件时生效：每行只输出 `name`。
        pure: bool,
        /// 仅在终端显示最终分数不低于此值的选手。
        min_screen: Option<f64>,
        /// 仅在输出到文件时生效：只写入最终分数不低于此值的选手。
        min_file: Option<f64>,
        /// 胜率小数位数。
        wr_precision: usize,
    },
    NamerPf {
        /// 每行一个名字组，组内可用 `+` 分隔。
        raw: String,
        /// 每个评分项的模拟场数。
        n: usize,
        /// 显式指定的 benchmark 线程数。
        threads: Option<usize>,
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
    ToDiy {
        /// 玩家名字列表（namerena raw 格式）。
        names: Vec<String>,
        /// 是否为文件批量模式。
        from_file: bool,
        /// 输出文件；未指定时输出到标准输出。
        out_file: Option<PathBuf>,
        /// 是否输出旧版 `+diy` 形式；默认输出 `+ol` 形式。
        old: bool,
    },
}

/// 解析后的 CLI 容器。
///
/// 当前只包一层 `command`，保留这个壳是为了给后续增加全局运行选项留出稳定扩展点，
/// 不必在 `main` 和执行模块之间反复改签名。
#[derive(Debug)]
pub struct ParsedCli {
    /// 解析完成后的 CLI 命令。
    pub command: ParsedCommand,
}
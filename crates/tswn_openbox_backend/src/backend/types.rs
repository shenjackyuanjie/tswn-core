//! 后端公共数据类型。
//!
//! 定义跨线程传递的事件类型（[`ProgressEvent`]）、输出模式（[`OutputMode`]）、
//! 公共基准选项（[`CommonBenchOptions`]）及各工具输入结构体
//! （[`BatchRateInput`]、[`PairInput`]）。

use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicBool};

#[derive(Debug, Clone)]
/// 自有任务事件，可跨线程转交给 GUI/CLI；日志内容无需借用计算任务。
///
/// Progress 是当前完成数/总数，Done 携带成功摘要或错误；仅收到进度不表示结果已落盘。
/// 事件回调本身同步执行，使用有界通道时调用方需持续消费，避免反向阻塞生产者。
pub enum ProgressEvent {
    Log(String),
    HighlightLog(String),
    SkillBoardLog(String),
    Progress { done: usize, total: usize },
    Done(Result<String, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 业务结果的输出形式：交互日志、JSONL 或纯文本。
pub enum OutputMode {
    Log,
    Jsonl,
    Pure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamerPfMetric {
    Pp,
    Pd,
    Qp,
    Qd,
    Sum,
}

impl NamerPfMetric {
    pub const ALL: [Self; 5] = [Self::Pp, Self::Pd, Self::Qp, Self::Qd, Self::Sum];

    pub fn label(self) -> &'static str {
        match self {
            Self::Pp => "pp",
            Self::Pd => "pd",
            Self::Qp => "qp",
            Self::Qd => "qd",
            Self::Sum => "sum",
        }
    }
}

#[derive(Debug, Clone)]
/// 批量胜率与配队共用选项，筛选不改变底层对局规则。
///
/// count 在任务入口至少按 1 处理；threads=None 表示自动，keep_rq=true 选择普通
/// 构建口径。屏幕/文件阈值分别作用于各自输出，配队得分可能超过 100；显示精度
/// 最多保留 9 位，不能据格式化结果反推内部计算精度。
pub struct CommonBenchOptions {
    pub count: usize,
    pub threads: Option<usize>,
    pub keep_rq: bool,
    pub verbose: bool,
    pub min_screen: Option<f64>,
    pub min_file: Option<f64>,
    pub wr_precision: usize,
}

#[derive(Debug, Clone)]
pub struct NamerPfMetricOptions {
    pub metric: NamerPfMetric,
    pub screen: bool,
    pub min_screen: Option<f64>,
    pub highlight_delta: Option<f64>,
    pub output_file: Option<PathBuf>,
    pub min_file: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct NamerPfSkillBoardOptions {
    pub screen: bool,
    pub output_file: Option<PathBuf>,
    /// 技能榜阈值配置文件；`None` 时按 GUI 惯例读取 `./setting/score_now.toml`。
    /// CLI 可显式指定路径，摆脱对当前目录配置的依赖。
    pub config: Option<PathBuf>,
}

#[derive(Debug, Clone)]
/// 一次评分任务的自有输入；raw 每行一个名字组合，metrics 决定需要计算和输出的指标。
/// 技能榜可额外加载阈值配置；cancel 只共享停止信号，结果通过任务事件交还。
pub struct NamerPfInput {
    pub raw: String,
    pub count: usize,
    pub threads: Option<usize>,
    pub keep_rq: bool,
    pub precision: usize,
    pub metrics: Vec<NamerPfMetricOptions>,
    pub skill_board: NamerPfSkillBoardOptions,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
/// 批量比较候选组与靶子的自有输入；文本不是文件路径，读取文件由上层完成。
/// target_factor_enabled 时 target_text 为带权 TOML，否则由分隔开关解析；
/// output_file 可创建目录并截断已有文件，cancel 为协作式停止请求。
pub struct BatchRateInput {
    pub target_text: String,
    pub player_text: String,
    pub target_factor_enabled: bool,
    pub target_double_plus: bool,
    pub player_double_plus: bool,
    pub show_matchups: bool,
    pub highlight_delta: Option<f64>,
    pub output_mode: OutputMode,
    pub output_file: Option<PathBuf>,
    pub options: CommonBenchOptions,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
/// 选手×队友×靶子的配队任务；head 控制头部搭档汇总数量。
///
/// 选手和队友在计算前按成员冻结构建，避免组合时再次获得同公会加成；展示标签
/// 保留原输入。带权开关分别选择靶子/队友 TOML，输出路径及取消令牌由任务持有。
pub struct PairInput {
    pub target_text: String,
    pub target_factor_enabled: bool,
    pub player_text: String,
    pub player_double_plus: bool,
    pub teammate_text: String,
    pub teammate_double_plus: bool,
    pub teammate_factor_enabled: bool,
    pub head: usize,
    pub detail_mode: PairDetailMode,
    pub detail_min: Option<f64>,
    pub highlight_delta: Option<f64>,
    pub output_mode: OutputMode,
    pub output_file: Option<PathBuf>,
    pub options: CommonBenchOptions,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairDetailMode {
    None,
    Every,
    Top,
}

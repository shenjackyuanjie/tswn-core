//! 后端公共数据类型。
//!
//! 定义跨线程传递的事件类型（[`ProgressEvent`]）、输出模式（[`OutputMode`]）、
//! 公共基准选项（[`CommonBenchOptions`]）及各工具输入结构体
//! （[`BatchRateInput`]、[`PairInput`]）。

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Log(String),
    Progress { done: usize, total: usize },
    Done(Result<String, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Log,
    Jsonl,
    Pure,
}

#[derive(Debug, Clone)]
pub struct CommonBenchOptions {
    pub count: usize,
    pub threads: Option<usize>,
    pub keep_rq: bool,
    pub verbose: bool,
    pub perf: bool,
    pub min_screen: Option<f64>,
    pub min_file: Option<f64>,
    pub wr_precision: usize,
}

#[derive(Debug, Clone)]
pub struct BatchRateInput {
    pub target_text: String,
    pub player_text: String,
    pub player_double_plus: bool,
    pub output_mode: OutputMode,
    pub output_file: Option<PathBuf>,
    pub options: CommonBenchOptions,
}

#[derive(Debug, Clone)]
pub struct PairInput {
    pub target_text: String,
    pub player_text: String,
    pub teammate_text: String,
    pub head: usize,
    pub output_mode: OutputMode,
    pub output_file: Option<PathBuf>,
    pub options: CommonBenchOptions,
}

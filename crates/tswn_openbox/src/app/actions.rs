//! 各工具的启动与事件轮询逻辑。
//!
//! 实现 `OpenboxApp` 的任务启动方法（`start_to_diy`、`start_batch_rate` 等），
//! 在独立线程中调用后端函数，并通过 `poll_events` 处理进度/日志/完成事件。

use std::ops::RangeInclusive;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, TryRecvError},
};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::backend::PairDetailMode;
use crate::backend::{
    self, BatchRateInput, CommonBenchOptions, NamerPfInput, NamerPfMetricOptions, NamerPfSkillBoardOptions, PairInput,
    ProgressEvent,
};

use super::state::{CountMode, OpenboxApp};
use super::target_presets::{load_selected_target_text, load_selected_teammate_text};
use super::widgets::OptionalFileOutput;

const MAX_EVENTS_PER_POLL: usize = 256;
const RUNNING_REPAINT_INTERVAL: Duration = Duration::from_millis(100);

impl OpenboxApp {
    pub fn stop_current_task(&mut self) {
        if let Some(token) = &self.cancel_token {
            token.store(true, Ordering::Relaxed);
            self.cancel_requested = true;
            self.status = "停止中".to_string();
        }
    }

    pub fn start_to_diy(&mut self) {
        let raw = match self.to_diy.names.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let output_file = match resolve_output_path(&self.to_diy.output) {
            Ok(path) => path,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };

        if self.to_diy.old && self.to_diy.minions {
            self.fail_before_start("to-diy: --old 与 --minions 不能同时使用。".to_string());
            return;
        }

        self.begin_task();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let old = self.to_diy.old;
        let minions = self.to_diy.minions;
        let details = self.to_diy.details && output_file.is_none();
        let cancel = self.cancel_token();
        std::thread::spawn(move || {
            let result = backend::run_to_diy(&raw, old, minions, details, output_file, &cancel);
            let _ = tx.send(ProgressEvent::Done(result));
        });
    }

    pub fn start_namer_pf(&mut self) {
        let raw = match self.namer_pf.names.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let mut metrics = Vec::with_capacity(self.namer_pf.metrics.len());
        for metric in &self.namer_pf.metrics {
            let min_screen = if metric.screen {
                match parse_optional_f64_at_least(&metric.min_screen, &format!("{} 屏幕阈值", metric.metric.label()), 0.0) {
                    Ok(value) => value,
                    Err(err) => {
                        self.fail_before_start(err);
                        return;
                    }
                }
            } else {
                None
            };
            let min_file = if metric.file_output.enabled {
                match parse_optional_f64_at_least(&metric.min_file, &format!("{} 文件阈值", metric.metric.label()), 0.0) {
                    Ok(value) => value,
                    Err(err) => {
                        self.fail_before_start(err);
                        return;
                    }
                }
            } else {
                None
            };
            let highlight_delta = if metric.screen {
                match parse_optional_f64_at_least(
                    &metric.highlight_delta,
                    &format!("{} 高亮超强名字", metric.metric.label()),
                    0.0,
                ) {
                    Ok(value) => value,
                    Err(err) => {
                        self.fail_before_start(err);
                        return;
                    }
                }
            } else {
                None
            };
            let output_file = match resolve_output_path(&metric.file_output) {
                Ok(path) => path,
                Err(err) => {
                    self.fail_before_start(format!("{}: {err}", metric.metric.label()));
                    return;
                }
            };
            metrics.push(NamerPfMetricOptions {
                metric: metric.metric,
                screen: metric.screen,
                min_screen,
                highlight_delta,
                output_file,
                min_file,
            });
        }

        let skill_board_output_file = match resolve_output_path(&self.namer_pf.skill_board.file_output) {
            Ok(path) => path,
            Err(err) => {
                self.fail_before_start(format!("技能榜: {err}"));
                return;
            }
        };

        self.begin_task();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let cancel = self.cancel_token();
        let input = NamerPfInput {
            raw,
            count: bench_count(self.namer_pf.count_mode, self.namer_pf.accuracy, self.namer_pf.count),
            threads: bench_threads(self.namer_pf.auto_threads, self.namer_pf.threads),
            keep_rq: self.namer_pf.keep_rq,
            precision: self.namer_pf.precision.min(9),
            metrics,
            skill_board: NamerPfSkillBoardOptions {
                screen: self.namer_pf.skill_board.screen,
                output_file: skill_board_output_file,
            },
            cancel,
        };
        std::thread::spawn(move || {
            backend::run_namer_pf(input, |event| {
                let _ = tx.send(event);
            });
        });
    }

    pub fn start_batch_rate(&mut self) {
        let target_text = match read_target_text(
            &self.batch_rate.targets,
            &self.batch_rate.target_presets,
            self.batch_rate.manual_targets,
        ) {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let player_text = match self.batch_rate.players.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let output_file = self.batch_rate.output.file_output.path();
        let min_screen = match parse_optional_f64_in_range(&self.batch_rate.output.min_screen, "日志阈值", 0.0..=100.0) {
            Ok(value) => value,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let min_file = match parse_optional_f64_in_range(&self.batch_rate.output.min_file, "文件阈值", 0.0..=100.0) {
            Ok(value) => value,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let highlight_delta = match parse_optional_f64_at_least(&self.batch_rate.highlight_delta, "高亮超强名字", 0.0) {
            Ok(value) => value,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };

        self.begin_task();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let cancel = self.cancel_token();
        let input = BatchRateInput {
            target_text,
            player_text,
            player_double_plus: self.batch_rate.double_plus,
            show_matchups: self.batch_rate.show_matchups,
            highlight_delta,
            output_mode: self.batch_rate.output.mode,
            output_file,
            options: CommonBenchOptions {
                count: bench_count(self.batch_rate.count_mode, self.batch_rate.accuracy, self.batch_rate.count),
                threads: bench_threads(self.batch_rate.auto_threads, self.batch_rate.threads),
                keep_rq: self.batch_rate.keep_rq,
                verbose: false,
                min_screen,
                min_file,
                wr_precision: self.batch_rate.output.precision.min(9),
            },
            cancel,
        };
        std::thread::spawn(move || {
            backend::run_batch_rate(input, |event| {
                let _ = tx.send(event);
            });
        });
    }

    pub fn start_pair(&mut self) {
        let target_text = match read_target_text(&self.pair.targets, &self.pair.target_presets, self.pair.manual_targets) {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let player_text = match self.pair.players.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let teammate_text =
            match read_teammate_text(&self.pair.teammates, &self.pair.teammate_presets, self.pair.manual_teammates) {
                Ok(raw) => raw,
                Err(err) => {
                    self.fail_before_start(err);
                    return;
                }
            };
        let head = if self.pair.manual_teammates {
            self.pair.head
        } else {
            self.pair.teammate_presets.selected().map(|preset| preset.head).unwrap_or(self.pair.head)
        };
        let output_file = self.pair.output.file_output.path();
        let min_screen = match parse_optional_f64_at_least(&self.pair.output.min_screen, "日志阈值", 0.0) {
            Ok(value) => value,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let min_file = match parse_optional_f64_at_least(&self.pair.output.min_file, "文件阈值", 0.0) {
            Ok(value) => value,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let detail_min = if self.pair.detail_mode == PairDetailMode::Every {
            match parse_optional_f64_in_range(&self.pair.detail_min, "cqp阈值", 0.0..=100.0) {
                Ok(value) => value,
                Err(err) => {
                    self.fail_before_start(err);
                    return;
                }
            }
        } else {
            None
        };
        let highlight_delta = match parse_optional_f64_at_least(&self.pair.highlight_delta, "高亮超强名字", 0.0) {
            Ok(value) => value,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };

        self.begin_task();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let cancel = self.cancel_token();
        let input = PairInput {
            target_text,
            player_text,
            teammate_text,
            head: head.max(1),
            detail_mode: self.pair.detail_mode,
            detail_min,
            highlight_delta,
            output_mode: self.pair.output.mode,
            output_file,
            options: CommonBenchOptions {
                count: bench_count(self.pair.count_mode, self.pair.accuracy, self.pair.count),
                threads: bench_threads(self.pair.auto_threads, self.pair.threads),
                keep_rq: self.pair.keep_rq,
                verbose: false,
                min_screen,
                min_file,
                wr_precision: self.pair.output.precision.min(9),
            },
            cancel,
        };
        std::thread::spawn(move || {
            backend::run_pair(input, |event| {
                let _ = tx.send(event);
            });
        });
    }

    pub fn begin_task(&mut self) {
        self.running = true;
        self.cancel_requested = false;
        self.cancel_token = Some(Arc::new(AtomicBool::new(false)));
        self.done = 0;
        self.total = 0;
        self.started_at = Some(Instant::now());
        self.rate_text = "--".to_string();
        self.eta_text = "--".to_string();
        self.log.clear();
        self.highlight_lines.clear();
        self.skill_board_lines.clear();
        self.status = "运行中".to_string();
    }

    pub fn fail_before_start(&mut self, err: String) {
        self.running = false;
        self.cancel_requested = false;
        self.cancel_token = None;
        self.done = 0;
        self.total = 0;
        self.started_at = None;
        self.rate_text = "--".to_string();
        self.eta_text = "--".to_string();
        self.rx = None;
        self.status = "失败".to_string();
        self.log.clear();
        self.highlight_lines.clear();
        self.skill_board_lines.clear();
        self.append_log(&err);
    }

    pub fn poll_events(&mut self, ctx: &egui::Context) {
        let mut processed_any = false;
        let mut hit_event_limit = false;
        if let Some(rx) = self.rx.take() {
            let mut keep_rx = true;
            for event_index in 0..MAX_EVENTS_PER_POLL {
                match rx.try_recv() {
                    Ok(event) => {
                        processed_any = true;
                        hit_event_limit = event_index + 1 == MAX_EVENTS_PER_POLL;
                        match event {
                            ProgressEvent::Log(line) => {
                                self.append_log(&line);
                            }
                            ProgressEvent::HighlightLog(line) => {
                                self.append_highlight_log(&line);
                            }
                            ProgressEvent::SkillBoardLog(line) => {
                                self.append_skill_board_log(&line);
                            }
                            ProgressEvent::Progress { done, total } => {
                                self.done = done;
                                self.total = total;
                                self.status = "运行中".to_string();
                                self.update_progress_stats();
                            }
                            ProgressEvent::Done(result) => {
                                self.running = false;
                                self.cancel_requested = false;
                                self.cancel_token = None;
                                self.status = match &result {
                                    Ok(_) => "完成".to_string(),
                                    Err(_) => "失败".to_string(),
                                };
                                match result {
                                    Ok(output) => self.append_log(&output),
                                    Err(err) => self.append_log(&err),
                                }
                                self.update_progress_stats();
                                keep_rx = false;
                                break;
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => {
                        hit_event_limit = false;
                        break;
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.running = false;
                        self.cancel_requested = false;
                        self.cancel_token = None;
                        keep_rx = false;
                        break;
                    }
                }
            }
            if keep_rx {
                self.rx = Some(rx);
            }
        }
        if self.running {
            self.update_progress_stats();
            ctx.request_repaint_after(RUNNING_REPAINT_INTERVAL);
        }
        if processed_any || hit_event_limit {
            ctx.request_repaint();
        }
    }

    pub fn update_progress_stats(&mut self) {
        let Some(started_at) = self.started_at else {
            return;
        };
        let elapsed = started_at.elapsed().as_secs_f64();
        if elapsed <= 0.0 || self.done == 0 {
            self.rate_text = "--".to_string();
            self.eta_text = "--".to_string();
            return;
        }
        let rate = self.done as f64 / elapsed;
        self.rate_text = format!("{rate:.2} 项/s");
        let remaining = self.total.saturating_sub(self.done) as f64;
        self.eta_text = if rate > 0.0 {
            format_duration(remaining / rate)
        } else {
            "--".to_string()
        };
    }

    pub fn append_log(&mut self, text: &str) { self.append_log_inner(text, false); }

    pub fn append_highlight_log(&mut self, text: &str) { self.append_log_inner(text, true); }

    pub fn append_skill_board_log(&mut self, text: &str) {
        let first_line = self.append_log_inner(text, false);
        self.skill_board_lines.insert(first_line);
    }

    fn append_log_inner(&mut self, text: &str, highlight_first_line: bool) -> usize {
        let trimmed = text.trim_end_matches('\n');
        if trimmed.is_empty() {
            return self.log.lines().count();
        }
        if !self.log.is_empty() && !self.log.ends_with('\n') {
            self.log.push('\n');
        }
        let first_line_index = self.log.lines().count();
        if highlight_first_line {
            self.highlight_lines.insert(first_line_index);
        }
        self.log.push_str(trimmed);
        self.log.push('\n');
        first_line_index
    }
}

impl OpenboxApp {
    fn cancel_token(&self) -> Arc<AtomicBool> {
        self.cancel_token.as_ref().expect("cancel token should be set after begin_task").clone()
    }
}

fn resolve_output_path(output: &OptionalFileOutput) -> Result<Option<std::path::PathBuf>, String> {
    match output.selected_path() {
        Some(path) => Ok(Some(path)),
        None if output.enabled => Err("请先选择输出文件。".to_string()),
        None => Ok(None),
    }
}

fn read_target_text(
    manual_source: &super::source::TextSource,
    presets: &super::target_presets::TargetPresetState,
    manual_targets: bool,
) -> Result<String, String> {
    if manual_targets {
        manual_source.read_all()
    } else {
        load_selected_target_text(presets)
    }
}

fn read_teammate_text(
    manual_source: &super::source::TextSource,
    presets: &super::target_presets::TeammatePresetState,
    manual_teammates: bool,
) -> Result<String, String> {
    if manual_teammates {
        manual_source.read_all()
    } else {
        load_selected_teammate_text(presets)
    }
}

fn bench_count(mode: CountMode, accuracy: super::state::AccuracyPreset, manual_count: usize) -> usize {
    match mode {
        CountMode::Accuracy => accuracy.count(),
        CountMode::Manual => manual_count.max(1),
    }
}

fn bench_threads(auto_threads: bool, threads: usize) -> Option<usize> { if auto_threads { None } else { non_zero(threads) } }

fn non_zero(value: usize) -> Option<usize> { if value == 0 { None } else { Some(value) } }

fn parse_optional_f64_in_range(raw: &str, field_name: &str, range: RangeInclusive<f64>) -> Result<Option<f64>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = parse_f64(trimmed, field_name)?;
    let min = *range.start();
    let max = *range.end();
    if value < min || value > max {
        return Err(format!("{field_name} 需要在 {min} 到 {max} 之间。"));
    }
    Ok(Some(value))
}

fn parse_optional_f64_at_least(raw: &str, field_name: &str, min: f64) -> Result<Option<f64>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = parse_f64(trimmed, field_name)?;
    if value < min {
        return Err(format!("{field_name} 需要大于等于 {min}。"));
    }
    Ok(Some(value))
}

fn parse_f64(raw: &str, field_name: &str) -> Result<f64, String> {
    let value = raw.parse::<f64>().map_err(|_| format!("{field_name} 需要是数字。"))?;
    if !value.is_finite() {
        return Err(format!("{field_name} 需要是有限数字。"));
    }
    Ok(value)
}

fn format_duration(secs: f64) -> String {
    if secs.is_nan() || secs.is_infinite() || secs < 0.0 {
        return "--".to_string();
    }
    let seconds = secs.round() as u64;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m{}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h{}m{}s", seconds / 3600, (seconds % 3600) / 60, seconds % 60)
    }
}

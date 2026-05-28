//! 各工具的启动与事件轮询逻辑。
//!
//! 实现 `OpenboxApp` 的任务启动方法（`start_to_diy`、`start_batch_rate` 等），
//! 在独立线程中调用后端函数，并通过 `poll_events` 处理进度/日志/完成事件。

use std::ops::RangeInclusive;
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui;

use crate::backend::{self, BatchRateInput, CommonBenchOptions, NamerPfInput, NamerPfMetricOptions, PairInput, ProgressEvent};

use super::state::OpenboxApp;
use super::widgets::OptionalFileOutput;

impl OpenboxApp {
    pub(crate) fn start_to_diy(&mut self) {
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
        std::thread::spawn(move || {
            let result = backend::run_to_diy(&raw, old, minions, details, output_file);
            let _ = tx.send(ProgressEvent::Done(result));
        });
    }

    pub(crate) fn start_namer_pf(&mut self) {
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
                output_file,
                min_file,
            });
        }

        self.begin_task();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let input = NamerPfInput {
            raw,
            count: self.namer_pf.count.max(1),
            threads: non_zero(self.namer_pf.threads),
            keep_rq: self.namer_pf.keep_rq,
            metrics,
        };
        std::thread::spawn(move || {
            backend::run_namer_pf(input, |event| {
                let _ = tx.send(event);
            });
        });
    }

    pub(crate) fn start_batch_rate(&mut self) {
        let target_text = match self.batch_rate.targets.read_all() {
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
        let output_file = match self.batch_rate.output.file_output.path() {
            Some(path) => Some(path),
            None => {
                self.fail_before_start("请先选择输出文件。".to_string());
                return;
            }
        };
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

        self.begin_task();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let input = BatchRateInput {
            target_text,
            player_text,
            player_double_plus: self.batch_rate.double_plus,
            output_mode: self.batch_rate.output.mode,
            output_file,
            options: CommonBenchOptions {
                count: self.batch_rate.count.max(1),
                threads: non_zero(self.batch_rate.threads),
                keep_rq: self.batch_rate.keep_rq,
                verbose: self.batch_rate.verbose,
                perf: self.batch_rate.perf,
                min_screen,
                min_file,
                wr_precision: self.batch_rate.output.precision.min(9),
            },
        };
        std::thread::spawn(move || {
            backend::run_batch_rate(input, |event| {
                let _ = tx.send(event);
            });
        });
    }

    pub(crate) fn start_pair(&mut self) {
        let target_text = match self.pair.targets.read_all() {
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
        let teammate_text = match self.pair.teammates.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let output_file = match self.pair.output.file_output.path() {
            Some(path) => Some(path),
            None => {
                self.fail_before_start("请先选择输出文件。".to_string());
                return;
            }
        };
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

        self.begin_task();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let input = PairInput {
            target_text,
            player_text,
            teammate_text,
            head: self.pair.head.max(1),
            output_mode: self.pair.output.mode,
            output_file,
            options: CommonBenchOptions {
                count: self.pair.count.max(1),
                threads: non_zero(self.pair.threads),
                keep_rq: self.pair.keep_rq,
                verbose: self.pair.verbose,
                perf: self.pair.perf,
                min_screen,
                min_file,
                wr_precision: self.pair.output.precision.min(9),
            },
        };
        std::thread::spawn(move || {
            backend::run_pair(input, |event| {
                let _ = tx.send(event);
            });
        });
    }

    pub(crate) fn begin_task(&mut self) {
        self.running = true;
        self.done = 0;
        self.total = 0;
        self.started_at = Some(Instant::now());
        self.rate_text = "--".to_string();
        self.eta_text = "--".to_string();
        self.log.clear();
        self.status = "运行中".to_string();
    }

    pub(crate) fn fail_before_start(&mut self, err: String) {
        self.running = false;
        self.done = 0;
        self.total = 0;
        self.started_at = None;
        self.rate_text = "--".to_string();
        self.eta_text = "--".to_string();
        self.rx = None;
        self.status = "失败".to_string();
        self.log.clear();
        self.append_log(&err);
    }

    pub(crate) fn poll_events(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.rx.take() {
            let mut keep_rx = true;
            while let Ok(event) = rx.try_recv() {
                match event {
                    ProgressEvent::Log(line) => {
                        self.append_log(&line);
                    }
                    ProgressEvent::Progress { done, total } => {
                        self.done = done;
                        self.total = total;
                        self.status = "运行中".to_string();
                        self.update_progress_stats();
                    }
                    ProgressEvent::Done(result) => {
                        self.running = false;
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
                    }
                }
                ctx.request_repaint();
            }
            if keep_rx {
                self.rx = Some(rx);
            }
        }
    }

    pub(crate) fn update_progress_stats(&mut self) {
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

    pub(crate) fn append_log(&mut self, text: &str) {
        let trimmed = text.trim_end_matches('\n');
        if trimmed.is_empty() {
            return;
        }
        if !self.log.is_empty() && !self.log.ends_with('\n') {
            self.log.push('\n');
        }
        self.log.push_str(trimmed);
        self.log.push('\n');
    }
}

fn resolve_output_path(output: &OptionalFileOutput) -> Result<Option<std::path::PathBuf>, String> {
    match output.selected_path() {
        Some(path) => Ok(Some(path)),
        None if output.enabled => Err("请先选择输出文件。".to_string()),
        None => Ok(None),
    }
}

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

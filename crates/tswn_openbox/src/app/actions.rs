//! 各工具的输入校验与任务参数组装。
//!
//! 实现 `OpenboxApp` 的任务启动方法（`start_to_diy`、`start_batch_rate` 等），
//! 生命周期与事件消费由 `task` 模块处理。

use std::ops::RangeInclusive;
use std::sync::{Arc, atomic::AtomicBool};

use tswn_openbox::backend::PairDetailMode;
use tswn_openbox::backend::{
    self, BatchRateInput, CommonBenchOptions, NamerPfInput, NamerPfMetricOptions, NamerPfSkillBoardOptions, PairInput,
    ProgressEvent,
};

use super::state::{CountMode, OpenboxApp};
use super::widgets::OptionalFileOutput;
use tswn_openbox::presets::{load_selected_target_text, load_selected_teammate_text};

impl OpenboxApp {
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
        let old = self.to_diy.old;
        let minions = self.to_diy.minions;
        let details = self.to_diy.details && output_file.is_none();
        let cancel = self.cancel_token();
        self.spawn_worker(move |feed| {
            let result = backend::run_to_diy_observed(
                &raw,
                old,
                minions,
                details,
                output_file,
                &cancel,
                Some(&|update| feed.result(update)),
            );
            feed.progress(ProgressEvent::Done(result));
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
                // GUI 始终走后端默认路径 ./setting/score_now.toml。
                config: None,
            },
            cancel,
        };
        self.spawn_worker(move |feed| {
            backend::run_namer_pf_observed(input, |event| feed.progress(event), Some(&|update| feed.result(update)));
        });
    }

    pub fn start_batch_rate(&mut self) {
        let (target_text, target_double_plus, target_factor_enabled) = if self.batch_rate.manual_targets {
            match self.batch_rate.targets.read_all() {
                Ok(raw) => (raw, self.batch_rate.manual_target_double_plus, false),
                Err(err) => {
                    self.fail_before_start(err);
                    return;
                }
            }
        } else {
            let selected = self.batch_rate.target_presets.selected();
            let target_double_plus = selected.is_some_and(|preset| preset.diy);
            let target_factor_enabled = selected.is_some_and(|preset| preset.factor_enabled);
            match load_selected_target_text(&self.batch_rate.target_presets) {
                Ok(raw) => (raw, target_double_plus, target_factor_enabled),
                Err(err) => {
                    self.fail_before_start(err);
                    return;
                }
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
        let cancel = self.cancel_token();
        let input = BatchRateInput {
            target_text,
            player_text,
            target_factor_enabled,
            target_double_plus,
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
        self.spawn_worker(move |feed| {
            backend::run_batch_rate_observed(input, |event| feed.progress(event), Some(&|update| feed.result(update)));
        });
    }

    pub fn start_pair(&mut self) {
        let (target_text, target_factor_enabled) = if self.pair.manual_targets {
            match self.pair.targets.read_all() {
                Ok(raw) => (raw, false),
                Err(err) => {
                    self.fail_before_start(err);
                    return;
                }
            }
        } else {
            let target_factor_enabled = self.pair.target_presets.selected().is_some_and(|preset| preset.factor_enabled);
            match load_selected_target_text(&self.pair.target_presets) {
                Ok(raw) => (raw, target_factor_enabled),
                Err(err) => {
                    self.fail_before_start(err);
                    return;
                }
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
        let teammate_factor_enabled =
            !self.pair.manual_teammates && self.pair.teammate_presets.selected().is_some_and(|preset| preset.factor_enabled);
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
        let cancel = self.cancel_token();
        let input = PairInput {
            target_text,
            target_factor_enabled,
            player_text,
            player_double_plus: self.pair.player_double_plus,
            teammate_text,
            teammate_double_plus: self.pair.teammate_double_plus,
            teammate_factor_enabled,
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
        self.spawn_worker(move |feed| {
            backend::run_pair_observed(input, |event| feed.progress(event), Some(&|update| feed.result(update)));
        });
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

fn read_teammate_text(
    manual_source: &super::source::TextSource,
    presets: &tswn_openbox::presets::TeammatePresetState,
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

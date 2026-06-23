//! 后端任务实现。
//!
//! 实现各工具的核心计算逻辑（`run_to_diy`、`run_namer_pf`、`run_batch_rate`、`run_pair`），
//! 通过 `Sender<ProgressEvent>` 向 GUI 线程实时推送进度日志和最终结果。

use std::cmp::Ordering as CmpOrdering;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Instant;

use tswn_core::cli_api;
use tswn_core::engine::storage::Storage;
use tswn_core::player::{Player, eval_name::WIN_RATE_EVAL_RQ};
use tswn_core::win_rate::resolve_win_rate_workers;

use super::format::{
    format_batch_file_record, format_batch_screen_log, format_pair_file_record, format_pair_screen_log, format_rate,
};
use super::parse::{parse_line_list, parse_namer_pf_groups, parse_player_groups_with_labels, parse_plus_separated_groups};
use super::score::{BatchRateSummary, BatchTargetOutcome, bench_batch_rate_for_group, namer_pf_score};
use super::skill_board::{SkillBoardConfig, evaluate_skill_board};
use super::types::{BatchRateInput, NamerPfInput, NamerPfMetric, NamerPfMetricOptions, OutputMode, PairInput, ProgressEvent};

const LOW_ACCURACY_OUTER_PARALLEL_LIMIT: usize = 1000;
const WORKER_EVENT_CHANNEL_CAPACITY: usize = 4096;

pub fn run_to_diy(
    raw: &str,
    old: bool,
    minions: bool,
    details: bool,
    output_file: Option<PathBuf>,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<String, String> {
    let names = parse_line_list(raw);
    if names.is_empty() {
        return Err("请输入至少一个名字。".to_string());
    }

    let mut out = String::new();
    for name in &names {
        if cancel.load(Ordering::Relaxed) {
            return Ok("已停止。".to_string());
        }
        let export = cli_api::to_diy(name, old, minions).map_err(|err| format!("导出 DIY 失败: {name}: {err}"))?;
        let _ = writeln!(out, "{export}");

        if details
            && names.len() == 1
            && let Some(detail_name) = single_to_diy_detail_name(name)
        {
            let storage = Storage::new_arc();
            let mut player = Player::new_from_namerena_raw(detail_name.to_string(), storage)
                .map_err(|err| format!("构建玩家失败: {detail_name}: {err}"))?;
            player.build();
            let status = player.get_status();
            let _ = writeln!(out);
            let _ = writeln!(out, "=== 原始信息 ===");
            let _ = writeln!(out, "名字: {}", player.id_name());
            let _ = writeln!(out, "队伍: {}", player.clan_name());
            let _ = writeln!(
                out,
                "八围: atk={} def={} spd={} agi={} mag={} res={} wis={} maxhp={}",
                status.attack,
                status.defense,
                status.speed,
                status.agility,
                status.magic,
                status.resistance,
                status.wisdom,
                status.max_hp,
            );
            let _ = writeln!(out, "技能: {}", player_diy_skill_object(&player));
            let _ = writeln!(out, "name_factor: {:.6}", player.get_name_factor());
        }
    }

    finish_output(output_file.as_deref(), out)
}

fn single_to_diy_detail_name(raw: &str) -> Option<String> {
    let groups = parse_namer_pf_groups(raw);
    match groups.as_slice() {
        [group] if group.len() == 1 => group.first().cloned(),
        _ => None,
    }
}
fn player_diy_skill_object(player: &Player) -> String {
    let diy = player.to_diy_compact();
    extract_diy_skill_object(&diy).unwrap_or("{}").to_string()
}

fn extract_diy_skill_object(diy: &str) -> Option<&str> {
    let attrs_start = diy.find("+diy[")? + "+diy[".len();
    let attrs_end = attrs_start + diy[attrs_start..].find(']')?;
    let object_start = attrs_end + 1;
    if !diy[object_start..].starts_with('{') {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in diy[object_start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = object_start + offset + ch.len_utf8();
                    return Some(&diy[object_start..end]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn run_namer_pf(input: NamerPfInput, send: impl Fn(ProgressEvent)) {
    let groups = parse_namer_pf_groups(&input.raw);
    if groups.is_empty() {
        send(ProgressEvent::Done(Err("namer-pf: 输入为空或无有效玩家。".to_string())));
        return;
    }
    if input.metrics.iter().all(|metric| !metric.screen && metric.output_file.is_none())
        && !input.skill_board.screen
        && input.skill_board.output_file.is_none()
    {
        send(ProgressEvent::Done(Err(
            "namer-pf: 请至少选择一个屏幕输出或输出文件。".to_string()
        )));
        return;
    }

    let mut outputs = Vec::with_capacity(input.metrics.len());
    for metric in &input.metrics {
        let output = match metric.output_file.as_deref() {
            Some(path) => match create_output_file(path) {
                Ok(file) => Some(file),
                Err(err) => {
                    send(ProgressEvent::Done(Err(err)));
                    return;
                }
            },
            None => None,
        };
        outputs.push(output);
    }
    let mut skill_board_output = match input.skill_board.output_file.as_deref() {
        Some(path) => match create_output_file(path) {
            Ok(file) => Some(file),
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        },
        None => None,
    };
    let skill_board_config = if input.skill_board.screen || skill_board_output.is_some() {
        match SkillBoardConfig::load_default() {
            Ok(config) => Some(config),
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        }
    } else {
        None
    };

    let n = input.count.max(1);
    let eval_rq = eval_rq(input.keep_rq);
    let precision = input.precision.min(9);
    let total = groups.len();
    let needs_skill_board_scores = skill_board_config.is_some();
    let needs_sum = metric_enabled(&input.metrics, NamerPfMetric::Sum);
    let needs_all_scores = needs_skill_board_scores || needs_sum;
    let needs_pp = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Pp);
    let needs_pd = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Pd);
    let needs_qp = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Qp);
    let needs_qd = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Qd);

    let score_settings = NamerPfScoreSettings {
        n,
        threads: input.threads,
        eval_rq,
        needs_pp,
        needs_pd,
        needs_qp,
        needs_qd,
    };
    let outer_workers = low_accuracy_outer_workers(n, total, input.threads);
    let completed = if outer_workers > 1 {
        let score_settings = score_settings.with_threads(Some(1));
        match run_namer_pf_outer_parallel(
            &groups,
            score_settings,
            outer_workers,
            Arc::clone(&input.cancel),
            |result| {
                emit_namer_pf_result(
                    &result,
                    &input.metrics,
                    &mut outputs,
                    SkillBoardEmitCfg {
                        config: skill_board_config.as_ref(),
                        output: &mut skill_board_output,
                        screen: input.skill_board.screen,
                    },
                    precision,
                    &send,
                )
            },
            |done| send(ProgressEvent::Progress { done, total }),
        ) {
            Ok(done) => done,
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        }
    } else {
        let mut completed = 0usize;
        for (index, group) in groups.iter().enumerate() {
            if input.cancel.load(Ordering::Relaxed) {
                send(ProgressEvent::Done(Ok("已停止。".to_string())));
                return;
            }
            let result = compute_namer_pf_result(index, group, score_settings);
            if let Err(err) = emit_namer_pf_result(
                &result,
                &input.metrics,
                &mut outputs,
                SkillBoardEmitCfg {
                    config: skill_board_config.as_ref(),
                    output: &mut skill_board_output,
                    screen: input.skill_board.screen,
                },
                precision,
                &send,
            ) {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
            completed = index + 1;
            send(ProgressEvent::Progress { done: completed, total });
        }
        completed
    };

    if input.cancel.load(Ordering::Relaxed) && completed < total {
        send(ProgressEvent::Done(Ok("已停止。".to_string())));
        return;
    }

    let mut written = input
        .metrics
        .iter()
        .filter_map(|metric| {
            metric
                .output_file
                .as_ref()
                .map(|path| format!("{} -> {}", metric.metric.label(), path.display()))
        })
        .collect::<Vec<_>>();
    if let Some(path) = input.skill_board.output_file.as_ref() {
        written.push(format!("技能榜 -> {}", path.display()));
    }
    let message = if written.is_empty() {
        "完成。".to_string()
    } else {
        format!("完成，结果已写入: {}", written.join("; "))
    };
    send(ProgressEvent::Done(Ok(message)));
}

fn metric_enabled(metrics: &[super::types::NamerPfMetricOptions], metric: NamerPfMetric) -> bool {
    metrics
        .iter()
        .any(|options| options.metric == metric && (options.screen || options.output_file.is_some()))
}

#[derive(Clone, Copy)]
struct NamerPfScoreSettings {
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
    needs_pp: bool,
    needs_pd: bool,
    needs_qp: bool,
    needs_qd: bool,
}

impl NamerPfScoreSettings {
    fn with_threads(self, threads: Option<usize>) -> Self { Self { threads, ..self } }
}

struct NamerPfJobResult {
    index: usize,
    group: Vec<String>,
    label: String,
    scores: NamerPfScores,
}

fn compute_namer_pf_result(index: usize, group: &[String], settings: NamerPfScoreSettings) -> NamerPfJobResult {
    let pp = if settings.needs_pp {
        namer_pf_score(group, "\u{0002}", false, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    let pd = if settings.needs_pd {
        namer_pf_score(group, "\u{0002}", true, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    let qp = if settings.needs_qp {
        namer_pf_score(group, "!", false, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    let qd = if settings.needs_qd {
        namer_pf_score(group, "!", true, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    let scores = NamerPfScores {
        pp,
        pd,
        qp,
        qd,
        sum: pp + pd + qp + qd,
    };
    NamerPfJobResult {
        index,
        group: group.to_vec(),
        label: group.join("+"),
        scores,
    }
}

struct SkillBoardEmitCfg<'a> {
    config: Option<&'a SkillBoardConfig>,
    output: &'a mut Option<File>,
    screen: bool,
}

fn emit_namer_pf_result(
    result: &NamerPfJobResult,
    metrics: &[NamerPfMetricOptions],
    outputs: &mut [Option<File>],
    skill_board: SkillBoardEmitCfg<'_>,
    precision: usize,
    send: &impl Fn(ProgressEvent),
) -> Result<(), String> {
    for (metric, output) in metrics.iter().zip(outputs.iter_mut()) {
        let score = result.scores.get(metric.metric);
        if metric.min_screen.is_none_or(|limit| score >= limit) && metric.screen {
            let score_text = format_rate(score, precision);
            let line = format!("{} {}:{}", result.label, metric.metric.label(), score_text);
            if should_highlight(score, metric.min_screen, metric.highlight_delta) {
                send(ProgressEvent::HighlightLog(line));
            } else {
                send(ProgressEvent::Log(line));
            }
        }
        if metric.min_file.is_none_or(|limit| score >= limit)
            && let Some(output) = output.as_mut()
            && let Err(err) = writeln!(output, "{} {}", format_rate(score, precision), result.label)
        {
            return Err(format!("写入输出文件失败: {err}"));
        }
    }
    if let Some(config) = skill_board.config {
        for line in evaluate_skill_board(&result.group, &result.scores, config) {
            let score_text = format_rate(line.score, precision);
            if skill_board.screen {
                send(ProgressEvent::SkillBoardLog(format!(
                    "{} {} {}",
                    line.title, score_text, result.label
                )));
            }
            if let Some(output) = skill_board.output.as_mut()
                && let Err(err) = writeln!(output, "{} {} {}", line.title, score_text, result.label)
            {
                return Err(format!("写入输出文件失败: {err}"));
            }
        }
    }
    Ok(())
}

fn run_namer_pf_outer_parallel(
    groups: &[Vec<String>],
    settings: NamerPfScoreSettings,
    workers: usize,
    cancel: Arc<AtomicBool>,
    mut emit: impl FnMut(NamerPfJobResult) -> Result<(), String>,
    mut report_progress: impl FnMut(usize),
) -> Result<usize, String> {
    let groups = Arc::new(groups.to_vec());
    let next = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::sync_channel(WORKER_EVENT_CHANNEL_CAPACITY);
    let worker_count = workers.min(groups.len()).max(1);
    let mut handles = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        let groups = Arc::clone(&groups);
        let next = Arc::clone(&next);
        let tx = tx.clone();
        let cancel = Arc::clone(&cancel);
        handles.push(std::thread::spawn(move || {
            loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= groups.len() {
                    break;
                }
                let result = compute_namer_pf_result(index, &groups[index], settings);
                if tx.send(result).is_err() {
                    break;
                }
            }
        }));
    }
    drop(tx);

    let mut pending = std::iter::repeat_with(|| None).take(groups.len()).collect::<Vec<_>>();
    let mut next_emit = 0usize;
    let mut completed = 0usize;
    let mut first_error = None;
    while let Ok(result) = rx.recv() {
        completed += 1;
        report_progress(completed);
        let index = result.index;
        if index < pending.len() {
            pending[index] = Some(result);
        }
        while next_emit < pending.len() {
            let Some(result) = pending[next_emit].take() else {
                break;
            };
            if first_error.is_none()
                && let Err(err) = emit(result)
            {
                first_error = Some(err);
                cancel.store(true, Ordering::Relaxed);
            }
            next_emit += 1;
        }
    }

    for handle in handles {
        handle.join().map_err(|_| "namer-pf 并行任务线程异常退出。".to_string())?;
    }
    if let Some(err) = first_error {
        return Err(err);
    }
    Ok(completed)
}

fn low_accuracy_outer_workers(n: usize, item_count: usize, threads: Option<usize>) -> usize {
    if n > LOW_ACCURACY_OUTER_PARALLEL_LIMIT || item_count <= 1 {
        return 1;
    }
    resolve_win_rate_workers(threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0), item_count)
}

#[derive(Debug, Clone, Copy)]
pub struct NamerPfScores {
    pub pp: f64,
    pub pd: f64,
    pub qp: f64,
    pub qd: f64,
    pub sum: f64,
}

impl NamerPfScores {
    fn get(&self, metric: NamerPfMetric) -> f64 {
        match metric {
            NamerPfMetric::Pp => self.pp,
            NamerPfMetric::Pd => self.pd,
            NamerPfMetric::Qp => self.qp,
            NamerPfMetric::Qd => self.qd,
            NamerPfMetric::Sum => self.sum,
        }
    }
}

pub fn run_batch_rate(input: BatchRateInput, send: impl Fn(ProgressEvent)) {
    let target_groups = parse_plus_separated_groups(&input.target_text);
    let (player_groups, player_labels) = parse_player_groups_with_labels(&input.player_text, input.player_double_plus);
    if target_groups.is_empty() {
        send(ProgressEvent::Done(Err("batch-rate: 靶子列表为空。".to_string())));
        return;
    }
    if player_groups.is_empty() {
        send(ProgressEvent::Done(Err("batch-rate: 选手列表为空。".to_string())));
        return;
    }

    let mut output = match input.output_file.as_deref() {
        Some(path) => match create_output_file(path) {
            Ok(file) => Some(file),
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        },
        None => None,
    };
    if output.is_none() {
        send(ProgressEvent::Log("未选择输出文件，本次只输出到日志。".to_string()));
    }

    let n = input.options.count.max(1);
    let eval_rq = eval_rq(input.options.keep_rq);
    let precision = input.options.wr_precision.min(9);
    let total = player_groups.len() * target_groups.len();
    let mut done = 0usize;

    let job_settings = BatchRateJobSettings {
        n,
        threads: input.options.threads,
        eval_rq,
        verbose: input.options.verbose,
        collect_details: input.show_matchups,
    };
    let outer_workers = low_accuracy_outer_workers(n, player_groups.len(), input.options.threads);
    if outer_workers > 1 {
        let job_settings = job_settings.with_threads(Some(1));
        if let Err(err) = run_batch_rate_outer_parallel(
            &player_groups,
            &player_labels,
            &target_groups,
            job_settings,
            outer_workers,
            Arc::clone(&input.cancel),
            || {
                done += 1;
                send(ProgressEvent::Progress { done, total });
            },
            |result| emit_batch_rate_result(&result, &input, &mut output, precision, &send),
        ) {
            send(ProgressEvent::Done(Err(err)));
            return;
        }
    } else {
        for (player, label) in player_groups.iter().zip(player_labels.iter()) {
            let result = compute_batch_rate_result(player, label, &target_groups, job_settings, &input.cancel, || {
                done += 1;
                send(ProgressEvent::Progress { done, total });
            });
            if let Err(err) = emit_batch_rate_result(&result, &input, &mut output, precision, &send) {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
            if input.cancel.load(Ordering::Relaxed) {
                send(ProgressEvent::Done(Ok("已停止。".to_string())));
                return;
            }
        }
    }

    if input.cancel.load(Ordering::Relaxed) {
        send(ProgressEvent::Done(Ok("已停止。".to_string())));
        return;
    }

    if let Err(err) = finalize_sorted_output_file(output.take(), input.output_file.as_deref(), input.output_mode) {
        send(ProgressEvent::Done(Err(err)));
        return;
    }

    let final_message = if let Some(path) = input.output_file.as_deref() {
        format!("完成，结果已写入: {}", path.display())
    } else {
        "完成。".to_string()
    };
    send(ProgressEvent::Done(Ok(final_message)));
}

#[derive(Clone, Copy)]
struct BatchRateJobSettings {
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
    verbose: bool,
    collect_details: bool,
}

impl BatchRateJobSettings {
    fn with_threads(self, threads: Option<usize>) -> Self { Self { threads, ..self } }
}

struct BatchRateJobResult {
    label: String,
    summary: BatchRateSummary,
    detail_rates: Vec<(f64, String)>,
}

enum BatchRateWorkerEvent {
    TargetDone,
    PlayerDone(BatchRateJobResult),
}

fn compute_batch_rate_result(
    player: &str,
    label: &str,
    target_groups: &[String],
    settings: BatchRateJobSettings,
    cancel: &AtomicBool,
    mut tick_target: impl FnMut(),
) -> BatchRateJobResult {
    let mut verbose = String::new();
    let mut detail_rates = Vec::new();
    let summary = bench_batch_rate_for_group(
        player,
        target_groups,
        settings.n,
        settings.threads,
        settings.eval_rq,
        settings.verbose,
        &mut verbose,
        cancel,
        |_, _, target, outcome| {
            if settings.collect_details
                && let BatchTargetOutcome::Rate { percent, .. } = &outcome
            {
                detail_rates.push((*percent, target.to_string()));
            }
            tick_target();
        },
    );

    BatchRateJobResult {
        label: label.to_string(),
        summary,
        detail_rates,
    }
}

fn emit_batch_rate_result(
    result: &BatchRateJobResult,
    input: &BatchRateInput,
    output: &mut Option<File>,
    precision: usize,
    send: &impl Fn(ProgressEvent),
) -> Result<(), String> {
    if input.options.min_file.is_none_or(|limit| result.summary.avg >= limit)
        && let Some(output) = output.as_mut()
    {
        let line = format_batch_file_record(input.output_mode, &result.label, result.summary.avg, precision);
        if let Err(err) = writeln!(output, "{line}") {
            return Err(format!("写入输出文件失败: {err}"));
        }
    }

    if input.options.min_screen.is_none_or(|limit| result.summary.avg >= limit) {
        let detail_rates = if input.show_matchups {
            result.detail_rates.as_slice()
        } else {
            &[]
        };
        let log = format_batch_screen_log(&result.label, result.summary.avg, detail_rates, precision);
        if should_highlight(result.summary.avg, input.options.min_screen, input.highlight_delta) {
            send(ProgressEvent::HighlightLog(log));
        } else {
            send(ProgressEvent::Log(log));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_batch_rate_outer_parallel(
    player_groups: &[String],
    player_labels: &[String],
    target_groups: &[String],
    settings: BatchRateJobSettings,
    workers: usize,
    cancel: Arc<AtomicBool>,
    mut tick_target: impl FnMut(),
    mut emit: impl FnMut(BatchRateJobResult) -> Result<(), String>,
) -> Result<usize, String> {
    let player_groups = Arc::new(player_groups.to_vec());
    let player_labels = Arc::new(player_labels.to_vec());
    let target_groups = Arc::new(target_groups.to_vec());
    let next = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::sync_channel(WORKER_EVENT_CHANNEL_CAPACITY);
    let worker_count = workers.min(player_groups.len()).max(1);
    let mut handles = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        let player_groups = Arc::clone(&player_groups);
        let player_labels = Arc::clone(&player_labels);
        let target_groups = Arc::clone(&target_groups);
        let next = Arc::clone(&next);
        let tx = tx.clone();
        let cancel = Arc::clone(&cancel);
        handles.push(std::thread::spawn(move || {
            loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= player_groups.len() {
                    break;
                }
                let result = compute_batch_rate_result(
                    &player_groups[index],
                    &player_labels[index],
                    &target_groups,
                    settings,
                    &cancel,
                    || {
                        let _ = tx.send(BatchRateWorkerEvent::TargetDone);
                    },
                );
                if tx.send(BatchRateWorkerEvent::PlayerDone(result)).is_err() {
                    break;
                }
            }
        }));
    }
    drop(tx);

    let mut completed_players = 0usize;
    let mut first_error = None;
    while let Ok(event) = rx.recv() {
        match event {
            BatchRateWorkerEvent::TargetDone => {
                if first_error.is_none() {
                    tick_target();
                }
            }
            BatchRateWorkerEvent::PlayerDone(result) => {
                completed_players += 1;
                if first_error.is_none()
                    && let Err(err) = emit(result)
                {
                    first_error = Some(err);
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        }
    }

    for handle in handles {
        handle.join().map_err(|_| "cqd/cqp 并行任务线程异常退出。".to_string())?;
    }
    if let Some(err) = first_error {
        return Err(err);
    }
    Ok(completed_players)
}

pub fn run_pair(input: PairInput, send: impl Fn(ProgressEvent)) {
    let target_groups = parse_plus_separated_groups(&input.target_text);
    let players = parse_line_list(&input.player_text);
    let teammates = parse_line_list(&input.teammate_text);
    if target_groups.is_empty() {
        send(ProgressEvent::Done(Err("pair: 靶子列表为空。".to_string())));
        return;
    }
    if players.is_empty() {
        send(ProgressEvent::Done(Err("pair: 选手列表为空。".to_string())));
        return;
    }
    if teammates.is_empty() {
        send(ProgressEvent::Done(Err("pair: 队友列表为空。".to_string())));
        return;
    }

    let mut output = match input.output_file.as_deref() {
        Some(path) => match create_output_file(path) {
            Ok(file) => Some(file),
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        },
        None => None,
    };
    if output.is_none() {
        send(ProgressEvent::Log("未选择输出文件，本次只输出到日志。".to_string()));
    }

    let n = input.options.count.max(1);
    let head = input.head.max(1);
    let eval_rq = eval_rq(input.options.keep_rq);
    let precision = input.options.wr_precision.min(9);
    let total = players.len() * teammates.len() * target_groups.len();
    let mut done = 0usize;

    for player in &players {
        let started = Instant::now();
        let converted_player = match player_to_ol(player) {
            Ok(value) => value,
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        };
        let mut pair_rates = Vec::with_capacity(teammates.len());
        let mut total_wins = 0usize;
        let mut total_battles = 0usize;
        let mut _total_valid_matchups = 0usize;
        let mut _total_skipped_matchups = 0usize;
        let mut verbose = String::new();

        for teammate in &teammates {
            let pair_group = format!("{converted_player}\n{teammate}");
            if input.options.verbose {
                let _ = writeln!(verbose, "teammate: {teammate}");
            }
            let summary = bench_batch_rate_for_group(
                &pair_group,
                &target_groups,
                n,
                input.options.threads,
                eval_rq,
                input.options.verbose,
                &mut verbose,
                &input.cancel,
                |_, _, _, _| {
                    done += 1;
                    send(ProgressEvent::Progress { done, total });
                },
            );
            if summary.valid_matchups > 0 {
                pair_rates.push((summary.avg, teammate.clone()));
            }
            total_wins += summary.wins;
            total_battles += summary.total;
            _total_valid_matchups += summary.valid_matchups;
            _total_skipped_matchups += summary.skipped_matchups;
            if input.cancel.load(Ordering::Relaxed) {
                break;
            }
        }

        pair_rates.sort_by(|a, b| b.0.total_cmp(&a.0));
        let selected_count = head.min(pair_rates.len());
        let final_score = pair_rates.iter().take(selected_count).map(|(rate, _)| *rate).sum::<f64>();
        let _elapsed = started.elapsed();
        let _aggregate_rate = total_wins as f64 * 100.0 / total_battles.max(1) as f64;

        if input.options.min_file.is_none_or(|limit| final_score >= limit)
            && let Some(output) = output.as_mut()
        {
            let line = format_pair_file_record(
                input.output_mode,
                player,
                final_score,
                selected_count,
                head,
                &pair_rates,
                precision,
            );
            if let Err(err) = writeln!(output, "{line}") {
                send(ProgressEvent::Done(Err(format!("写入输出文件失败: {err}"))));
                return;
            }
        }

        if input.options.min_screen.is_none_or(|limit| final_score >= limit) {
            let log = format_pair_screen_log(
                player,
                final_score,
                selected_count,
                &pair_rates,
                input.detail_mode,
                input.detail_min,
                precision,
            );
            if should_highlight(final_score, input.options.min_screen, input.highlight_delta) {
                send(ProgressEvent::HighlightLog(log));
            } else {
                send(ProgressEvent::Log(log));
            }
        }
        if input.cancel.load(Ordering::Relaxed) {
            send(ProgressEvent::Done(Ok("已停止。".to_string())));
            return;
        }
    }

    if let Err(err) = finalize_sorted_output_file(output.take(), input.output_file.as_deref(), input.output_mode) {
        send(ProgressEvent::Done(Err(err)));
        return;
    }

    let final_message = if let Some(path) = input.output_file.as_deref() {
        format!("完成，结果已写入: {}", path.display())
    } else {
        "完成。".to_string()
    };
    send(ProgressEvent::Done(Ok(final_message)));
}

fn should_highlight(score: f64, min_screen: Option<f64>, highlight_delta: Option<f64>) -> bool {
    highlight_delta.is_some_and(|delta| score >= min_screen.unwrap_or(0.0) + delta)
}

fn finish_output(output_file: Option<&Path>, out: String) -> Result<String, String> {
    match output_file {
        Some(path) => {
            let mut file = create_output_file(path)?;
            file.write_all(out.as_bytes())
                .and_then(|_| file.flush())
                .map_err(|err| format!("写入输出文件失败: {}: {err}", path.display()))?;
            Ok(format!("完成，结果已写入: {}", path.display()))
        }
        None => Ok(out),
    }
}

fn create_output_file(path: &Path) -> Result<File, String> {
    if path.file_name().is_none() {
        return Err(format!("输出路径必须包含文件名: {}", path.display()));
    }
    if path.exists() && path.is_dir() {
        return Err(format!("输出路径不能是目录: {}", path.display()));
    }
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty())
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("创建输出目录失败: {}: {err}", parent.display()))?;
    }
    File::create(path).map_err(|err| format!("打开输出文件失败: {}: {err}", path.display()))
}

fn finalize_sorted_output_file(mut output: Option<File>, path: Option<&Path>, mode: OutputMode) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(file) = output.as_mut() {
        file.flush().map_err(|err| format!("刷新输出文件失败: {}: {err}", path.display()))?;
    }
    drop(output);
    sort_score_output_file(path, mode)
}

fn sort_score_output_file(path: &Path, mode: OutputMode) -> Result<(), String> {
    if mode == OutputMode::Pure {
        return Ok(());
    }
    let content = fs::read_to_string(path).map_err(|err| format!("读取输出文件失败: {}: {err}", path.display()))?;
    let mut lines = content.lines().filter(|line| !line.trim().is_empty()).collect::<Vec<_>>();
    lines.sort_by(|left, right| compare_score_output_lines(left, right, mode));
    let mut sorted = lines.join("\n");
    if !sorted.is_empty() {
        sorted.push('\n');
    }
    fs::write(path, sorted).map_err(|err| format!("写入排序输出文件失败: {}: {err}", path.display()))
}

fn compare_score_output_lines(left: &&str, right: &&str, mode: OutputMode) -> CmpOrdering {
    match (score_output_line_value(left, mode), score_output_line_value(right, mode)) {
        (Some(left_score), Some(right_score)) => right_score.total_cmp(&left_score).then_with(|| left.cmp(right)),
        (Some(_), None) => CmpOrdering::Less,
        (None, Some(_)) => CmpOrdering::Greater,
        (None, None) => left.cmp(right),
    }
}

fn score_output_line_value(line: &str, mode: OutputMode) -> Option<f64> {
    match mode {
        OutputMode::Log => line.split_whitespace().next()?.parse().ok(),
        OutputMode::Jsonl => {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            value
                .get("avg_win_rate")
                .or_else(|| value.get("score"))
                .and_then(serde_json::Value::as_f64)
        }
        OutputMode::Pure => None,
    }
}

fn eval_rq(keep_rq: bool) -> f64 {
    if keep_rq {
        tswn_core::player::eval_name::DEFAULT_EVAL_RQ
    } else {
        WIN_RATE_EVAL_RQ
    }
}

fn player_to_ol(raw: &str) -> Result<String, String> {
    if raw.contains("+diy[") || raw.contains("+ol:") {
        return Ok(raw.to_string());
    }
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw(raw.to_string(), storage)
        .map_err(|err| format!("转换 player-list 名字为 +ol 失败: {raw}: {err}"))?;
    player.build();
    Ok(player.to_ol_json())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use super::{OutputMode, compare_score_output_lines, run_to_diy, score_output_line_value};

    #[test]
    fn log_output_lines_sort_by_score_descending() {
        let mut lines = vec!["12.000 beta", "99.500 alpha", "bad line", "99.500 gamma"];
        lines.sort_by(|left, right| compare_score_output_lines(left, right, OutputMode::Log));
        assert_eq!(lines, vec!["99.500 alpha", "99.500 gamma", "12.000 beta", "bad line"]);
    }

    #[test]
    fn jsonl_output_line_score_accepts_batch_and_pair_keys() {
        assert_eq!(
            score_output_line_value(r#"{"label":"a","avg_win_rate":64.25}"#, OutputMode::Jsonl),
            Some(64.25)
        );
        assert_eq!(
            score_output_line_value(r#"{"label":"a","score":300.0}"#, OutputMode::Jsonl),
            Some(300.0)
        );
    }

    #[test]
    fn to_diy_details_include_skill_object() {
        let cancel = AtomicBool::new(false);
        let output = run_to_diy(
            r#"mario+diy[72,39,69,76,67,66,0,84]{"sklfire":5,"sklheal":"40+30"}"#,
            true,
            false,
            true,
            None,
            &cancel,
        )
        .unwrap();

        assert!(output.contains(r#"技能: {"sklfire":5,"sklheal":"40+30"}"#));
    }

    #[test]
    fn to_diy_plus_line_exports_team_group() {
        let cancel = AtomicBool::new(false);
        let output = run_to_diy("1@a\n2@a\n1@a+2@a", true, false, false, None, &cancel).unwrap();
        let lines = output.lines().collect::<Vec<_>>();

        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("1@a+diy["));
        assert!(lines[1].starts_with("2@a+diy["));
        assert!(lines[2].starts_with("1@a+diy["));
        assert!(lines[2].contains("+2@a+diy["));
    }
}

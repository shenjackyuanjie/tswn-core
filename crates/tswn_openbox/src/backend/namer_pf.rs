//! 批量评分任务：增量指标、技能榜与有序输出。

use super::format::{format_rate, should_highlight};
use super::live::{EntryKind, ResultEntry, ResultFinish, ResultKind, ResultObserver, ResultUpdate};
use super::output::create_output_file;
use super::parse::parse_namer_pf_groups;
use super::score::{NamerPfScores, eval_rq, namer_pf_score, outer_thread_spec};
use super::skill_board::{SkillBoardConfig, SkillBoardLine, evaluate_skill_board};
use super::types::{NamerPfInput, NamerPfMetric, NamerPfMetricOptions, ProgressEvent};
use std::fs::File;
use std::io::Write as _;
use std::sync::atomic::Ordering;
use tswn_core::bench_sched::{low_accuracy_outer_workers, run_outer_parallel_ordered};

pub fn run_namer_pf(input: NamerPfInput, send: impl Fn(ProgressEvent)) { run_namer_pf_observed(input, send, None); }

/// GUI 可选增量观察接口；None 保留原来的输出契约。
pub fn run_namer_pf_observed(input: NamerPfInput, send: impl Fn(ProgressEvent), observer: ResultObserver<'_>) {
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
        // GUI 不传 config，按 ./setting/score_now.toml 惯例加载；CLI 可显式指定。
        let loaded = match input.skill_board.config.as_deref() {
            Some(path) => SkillBoardConfig::load(path),
            None => SkillBoardConfig::load_default(),
        };
        match loaded {
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
    let compute = |index: usize, group: &[String], settings: NamerPfScoreSettings| {
        let label = group.join("+");
        let mut visible = false;
        let mut result = compute_namer_pf_result(group, settings, |metric, value| {
            let Some(observer) = observer else { return };
            let Some(options) = input.metrics.iter().find(|option| option.metric == metric && option.screen) else {
                return;
            };
            if options.min_screen.is_some_and(|min| value < min) {
                return;
            }
            visible = true;
            let mut update = ResultUpdate::new(index, &label, ResultKind::Scores, precision);
            let mut entry = ResultEntry::number(
                NamerPfMetric::ALL.iter().position(|m| *m == metric).unwrap(),
                metric.label().to_owned(),
                value,
            );
            if should_highlight(value, options.min_screen, options.highlight_delta) {
                entry.kind = EntryKind::Highlight;
            }
            update.entries.push(entry);
            observer(update);
        });
        if let Some(config) = skill_board_config.as_ref() {
            result.skill_lines = evaluate_skill_board(group, &result.scores, config);
        }
        if let Some(observer) = observer {
            let mut update = ResultUpdate::new(index, &label, ResultKind::Scores, precision);
            if input.skill_board.screen {
                for (i, line) in result.skill_lines.iter().enumerate() {
                    let mut entry = ResultEntry::number(5 + i, line.title.clone(), line.score);
                    entry.kind = EntryKind::SkillBoard;
                    update.entries.push(entry);
                    visible = true;
                }
            }
            update.finish = Some(ResultFinish {
                score: None,
                visible,
                highlight: false,
            });
            observer(update);
        }
        result
    };
    let outer_workers = low_accuracy_outer_workers(n, total, outer_thread_spec(input.threads));
    let completed = if outer_workers > 1 {
        let score_settings = score_settings.with_threads(Some(1));
        let mut progress_done = 0usize;
        match run_outer_parallel_ordered(
            &groups,
            outer_workers,
            &input.cancel,
            |index, group, tick| {
                let result = compute(index, group, score_settings);
                tick();
                result
            },
            || {
                progress_done += 1;
                send(ProgressEvent::Progress {
                    done: progress_done,
                    total,
                });
            },
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
                    observer.is_none().then_some(&send as &dyn Fn(ProgressEvent)),
                )
            },
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
            let result = compute(index, group, score_settings);
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
                observer.is_none().then_some(&send as &dyn Fn(ProgressEvent)),
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
    label: String,
    scores: NamerPfScores,
    skill_lines: Vec<SkillBoardLine>,
}

fn compute_namer_pf_result(
    group: &[String],
    settings: NamerPfScoreSettings,
    mut metric_done: impl FnMut(NamerPfMetric, f64),
) -> NamerPfJobResult {
    let pp = if settings.needs_pp {
        namer_pf_score(group, "\u{0002}", false, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    if settings.needs_pp {
        metric_done(NamerPfMetric::Pp, pp);
    }
    let pd = if settings.needs_pd {
        namer_pf_score(group, "\u{0002}", true, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    if settings.needs_pd {
        metric_done(NamerPfMetric::Pd, pd);
    }
    let qp = if settings.needs_qp {
        namer_pf_score(group, "!", false, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    if settings.needs_qp {
        metric_done(NamerPfMetric::Qp, qp);
    }
    let qd = if settings.needs_qd {
        namer_pf_score(group, "!", true, settings.n, settings.threads, settings.eval_rq)
    } else {
        0.0
    };
    if settings.needs_qd {
        metric_done(NamerPfMetric::Qd, qd);
    }
    let scores = NamerPfScores {
        pp,
        pd,
        qp,
        qd,
        sum: pp + pd + qp + qd,
    };
    metric_done(NamerPfMetric::Sum, scores.sum);
    NamerPfJobResult {
        label: group.join("+"),
        scores,
        skill_lines: Vec::new(),
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
    send: Option<&dyn Fn(ProgressEvent)>,
) -> Result<(), String> {
    for (metric, output) in metrics.iter().zip(outputs.iter_mut()) {
        let score = result.scores.get(metric.metric);
        if let Some(send) = send.filter(|_| metric.screen && metric.min_screen.is_none_or(|limit| score >= limit)) {
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
    if skill_board.config.is_some() {
        for line in &result.skill_lines {
            let score_text = format_rate(line.score, precision);
            if skill_board.screen
                && let Some(send) = send
            {
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

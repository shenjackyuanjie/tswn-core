//! 评分 benchmark 与 `namer-pf` 的实现。
//!
//! 这一块的共同点是都依赖 JS 风格的 score profile 生成方式：
//! - 每一轮都会构造一份新的 profile 输入；
//! - 单人、双同名目标在 profile 数量上有特殊规则；
//! - 由于模板几乎不复用，必须避开全局缓存以免内存线性膨胀。

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use tswn_core::bench_sched::{low_accuracy_outer_workers, run_outer_parallel_ordered};
use tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::runtime::{
    RuntimeBatchSummary, RuntimeRunner, runtime_score, runtime_score_range, runtime_score_range_timed, runtime_score_timed,
};
use tswn_core::win_rate::WinRateTiming;

use crate::args::{BenchThreadMode, NamerPfMetric, NamerPfMetricSpec, NamerPfMode};

use super::common::{BenchSummary, thread_spec};
use super::output::{format_rate, print_perf_lines};
use super::skill_board::{SkillBoardConfig, evaluate_skill_board};

/// 标准 score benchmark 入口，同时输出普通评分与 `!评分`。
pub(super) fn run_bench_score(
    raw: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    perf: bool,
    buckets_step: Option<usize>,
) {
    let (groups, _) = RuntimeRunner::split_namerena_into_groups(raw.to_string());
    let target_group = groups.into_iter().next().unwrap_or_default();
    let target_count = target_group.len();
    if target_count == 0 {
        eprintln!("评分: 无目标玩家");
        return;
    }

    println!("=== 实力评分测试 ({n} 场) ===");
    println!("目标: {}", target_group.join(", "));
    println!("info: {target_count}");

    print!("[普通评分] ");
    let normal = if let Some(step) = buckets_step.filter(|step| *step > 0) {
        run_bench_score_with_bucket_output(&target_group, "\u{0002}", n, step, perf)
    } else {
        run_bench_score_inner(
            &target_group,
            "\u{0002}",
            n,
            mode,
            threads,
            WIN_RATE_EVAL_RQ,
            ScoreBenchOptions {
                show_progress: true,
                timed: perf,
            },
        )
    };
    let ns = normal.wins as f64 * 10_000.0 / normal.total.max(1) as f64;
    println!("普通评分: {:.0} / 10000  ({}/{})", ns, normal.wins, normal.total);
    if perf {
        print_perf_lines(normal.elapsed, normal.timing, normal.total);
    }

    print!("[!评分]    ");
    let bang = if let Some(step) = buckets_step.filter(|step| *step > 0) {
        run_bench_score_with_bucket_output(&target_group, "!", n, step, perf)
    } else {
        run_bench_score_inner(
            &target_group,
            "!",
            n,
            mode,
            threads,
            WIN_RATE_EVAL_RQ,
            ScoreBenchOptions {
                show_progress: true,
                timed: perf,
            },
        )
    };
    let bs = bang.wins as f64 * 10_000.0 / bang.total.max(1) as f64;
    println!("!评分:     {:.0} / 10000  ({}/{})", bs, bang.wins, bang.total);
    if perf {
        print_perf_lines(bang.elapsed, bang.timing, bang.total);
    }
}

/// 分段输出 score 累积结果。
fn run_bench_score_with_bucket_output(
    target_group: &[String],
    modifier: &str,
    n: usize,
    step: usize,
    timed: bool,
) -> BenchSummary {
    let started_at = Instant::now();
    let mut accumulated = RuntimeBatchSummary::default();
    let mut offset = 0usize;
    while offset < n {
        let chunk_end = (offset + step.max(1)).min(n);
        let chunk = if timed {
            runtime_score_range_timed(target_group, modifier, offset, chunk_end, WIN_RATE_EVAL_RQ)
        } else {
            runtime_score_range(target_group, modifier, offset, chunk_end, WIN_RATE_EVAL_RQ)
        };
        match chunk {
            Ok(chunk) => accumulated.merge(chunk),
            Err(error) => {
                eprintln!("分段 [{offset}, {chunk_end}) 评分失败: {error}");
                break;
            }
        }
        println!(
            "评分(分段): {:.0} / 10000  ({}/{})",
            accumulated.score_10000(),
            accumulated.wins,
            accumulated.total,
        );
        offset = chunk_end;
    }
    BenchSummary {
        wins: accumulated.wins,
        total: accumulated.total,
        timing: accumulated.timing,
        elapsed: started_at.elapsed(),
    }
}

/// score benchmark 执行器的输出与计时开关。
#[derive(Debug, Clone, Copy)]
struct ScoreBenchOptions {
    /// 是否在跑完后打印进度行。
    show_progress: bool,
    /// 是否逐场统计 init / fight 耗时（只有 `--perf` 需要）。
    timed: bool,
}

/// score benchmark 的统一执行器。
fn run_bench_score_inner(
    target_group: &[String],
    modifier: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
    options: ScoreBenchOptions,
) -> BenchSummary {
    let ScoreBenchOptions { show_progress, timed } = options;
    let started_at = Instant::now();
    let thread = match mode {
        BenchThreadMode::SingleThread => 1,
        BenchThreadMode::Parallel => thread_spec(threads),
    };
    let scored = if timed {
        runtime_score_timed(target_group, modifier, n, eval_rq, thread)
    } else {
        runtime_score(target_group, modifier, n, eval_rq, thread)
    };
    let summary = match scored {
        Ok(summary) => summary,
        Err(error) => {
            eprintln!("执行 Runtime 评分失败: {error}");
            return BenchSummary {
                wins: 0,
                total: 0,
                timing: WinRateTiming::default(),
                elapsed: started_at.elapsed(),
            };
        }
    };
    if show_progress && n >= 100 {
        println!("  进度: {}/{}", summary.total, n);
    }
    BenchSummary {
        wins: summary.wins,
        total: summary.total,
        timing: summary.timing,
        elapsed: started_at.elapsed(),
    }
}

/// `namer-pf` 的输出行为：屏幕总开关与技能榜配置 / 输出文件。
///
/// 对应 openbox GUI 里 namer-pf 面板的输出区；屏幕输出由 `no_screen` 统一开关，
/// 不逐指标配置（对齐既定设计）。
#[derive(Debug, Clone)]
pub struct NamerPfOutputOptions {
    pub no_screen: bool,
    pub skill_board_config: Option<PathBuf>,
    pub skill_board_output: Option<PathBuf>,
}

/// `namer-pf` 入口，输出形态与 tswn_openbox GUI 的 `run_namer_pf` 对齐。
///
/// 与 GUI 的差异只在交互层：GUI 的停止按钮/取消语义在 CLI 不适用；
/// 屏幕输出由顶层 `no_screen` 统一开关，不做逐指标屏幕复选框。
pub fn run_namer_pf(
    raw: &str,
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
    precision: usize,
    metrics: &[NamerPfMetricSpec],
    output: NamerPfOutputOptions,
) {
    let NamerPfOutputOptions {
        no_screen,
        skill_board_config,
        skill_board_output,
    } = output;
    let groups = parse_plus_separated_groups(raw);
    if groups.is_empty() {
        eprintln!("namer-pf: 输入为空或无有效玩家");
        std::process::exit(1);
    }

    // args 层已把 `--metric` 归一化为 pp/pd/qp/qd/sum 顺序；这里再防御性排序一次，
    // 保证输出顺序与命令行里的传入顺序无关（GUI 固定按 ALL 顺序输出）。
    let mut ordered = metrics.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|spec| NamerPfMetric::ALL.iter().position(|metric| *metric == spec.metric).unwrap_or(usize::MAX));

    // 输出文件统一在开跑前创建/截哑，语义与 GUI 的 create_output_file 一致：
    // 路径缺文件名、指向目录或父目录不存在时直接失败退出；中途失败也会留下空文件。
    let mut outputs = Vec::with_capacity(ordered.len());
    for spec in &ordered {
        let file = match spec.output_file.as_deref() {
            Some(path) => match create_output_file(path) {
                Ok(file) => Some(file),
                Err(err) => {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            },
            None => None,
        };
        outputs.push(file);
    }
    let mut skill_board_file = match skill_board_output.as_deref() {
        Some(path) => match create_output_file(path) {
            Ok(file) => Some(file),
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        },
        None => None,
    };
    let skill_board = match skill_board_config.as_deref() {
        Some(path) => match SkillBoardConfig::load(path) {
            Ok(config) => Some(config),
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        },
        None => None,
    };

    // sum 被选中或技能榜开启时，四项基础评分必须全部计算（GUI needs_all_scores 语义）；
    // 否则只算真正有出处的指标；既不写屏幕也不写文件的指标没有计算必要。
    let needs_all = skill_board.is_some()
        || ordered
            .iter()
            .any(|spec| spec.metric == NamerPfMetric::Sum && metric_emits(spec, no_screen));
    let selected = |metric: NamerPfMetric| ordered.iter().any(|spec| spec.metric == metric && metric_emits(spec, no_screen));
    let score_settings = NamerPfScoreSettings {
        n,
        threads,
        eval_rq,
        mode: BenchThreadMode::Parallel,
        needs_pp: needs_all || selected(NamerPfMetric::Pp),
        needs_pd: needs_all || selected(NamerPfMetric::Pd),
        needs_qp: needs_all || selected(NamerPfMetric::Qp),
        needs_qd: needs_all || selected(NamerPfMetric::Qd),
    };

    // 低精度（1%/10%）档位且有多组输入时，外层按组并行、内层单线程，
    // 比让每组的 4 个 score 各自反复起线程更划算；其余情况维持原有内层并行。
    let outer_workers = low_accuracy_outer_workers(n, groups.len(), thread_spec(threads));
    let mut emit_job = |result: NamerPfJobResult| -> Result<(), String> {
        emit_namer_pf_result(
            &result,
            &ordered,
            &mut outputs,
            skill_board.as_ref(),
            &mut skill_board_file,
            precision,
            no_screen,
        )
    };
    if outer_workers > 1 {
        let cancel = AtomicBool::new(false);
        let completed = run_outer_parallel_ordered(
            &groups,
            outer_workers,
            &cancel,
            |_, group, _| compute_namer_pf_result(group, score_settings.with_mode(BenchThreadMode::SingleThread)),
            || {},
            emit_job,
        );
        if let Err(err) = completed {
            eprintln!("{err}");
            std::process::exit(1);
        }
    } else {
        for group in &groups {
            let result = compute_namer_pf_result(group, score_settings);
            if let Err(err) = emit_job(result) {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
    }
}

/// `namer-pf` 单组计算的公共参数（对齐 GUI 的 NamerPfScoreSettings）。
#[derive(Debug, Clone, Copy)]
struct NamerPfScoreSettings {
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
    mode: BenchThreadMode,
    needs_pp: bool,
    needs_pd: bool,
    needs_qp: bool,
    needs_qd: bool,
}

impl NamerPfScoreSettings {
    /// 外层并行时切换为内层单线程，避免 worker 之间再起线程互相干扰。
    fn with_mode(self, mode: BenchThreadMode) -> Self { Self { mode, ..self } }
}

/// 单组名字的计算结果：原始组（供技能榜导出 DIY）、`+` 连接标签与五项评分。
struct NamerPfJobResult {
    group: Vec<String>,
    label: String,
    scores: NamerPfScores,
}

/// `namer-pf` 五项评分；未参与计算的项为 `0.0`（GUI 同样语义）。
#[derive(Debug, Clone, Copy)]
pub(super) struct NamerPfScores {
    pub pp: f64,
    pub pd: f64,
    pub qp: f64,
    pub qd: f64,
    pub sum: f64,
}

impl NamerPfScores {
    fn get(self, metric: NamerPfMetric) -> f64 {
        match metric {
            NamerPfMetric::Pp => self.pp,
            NamerPfMetric::Pd => self.pd,
            NamerPfMetric::Qp => self.qp,
            NamerPfMetric::Qd => self.qd,
            NamerPfMetric::Sum => self.sum,
        }
    }
}

/// 计算单组名字的四项基础评分与派生 `sum`（对齐 GUI compute_namer_pf_result）。
fn compute_namer_pf_result(group: &[String], settings: NamerPfScoreSettings) -> NamerPfJobResult {
    let NamerPfScoreSettings {
        n,
        threads,
        eval_rq,
        mode,
        needs_pp,
        needs_pd,
        needs_qp,
        needs_qd,
    } = settings;
    // 四个基础项的 modifier / duplicate 映射沿用 args 层 NamerPfMode 的单一出处，
    // 取值与 GUI 硬编码的 ("\u{0002}", false) 等完全一致。
    let run_base = |base: NamerPfMode| {
        let (modifier, duplicate) = base.score_params();
        namer_pf_score(group, modifier, duplicate, n, mode, threads, eval_rq)
    };
    let pp = if needs_pp { run_base(NamerPfMode::Pp) } else { 0.0 };
    let pd = if needs_pd { run_base(NamerPfMode::Pd) } else { 0.0 };
    let qp = if needs_qp { run_base(NamerPfMode::Qp) } else { 0.0 };
    let qd = if needs_qd { run_base(NamerPfMode::Qd) } else { 0.0 };
    NamerPfJobResult {
        group: group.to_vec(),
        label: group.join("+"),
        scores: NamerPfScores {
            pp,
            pd,
            qp,
            qd,
            sum: pp + pd + qp + qd,
        },
    }
}

/// 指标是否会产生任何输出：屏幕全局开启，或配置了输出文件。
fn metric_emits(spec: &NamerPfMetricSpec, no_screen: bool) -> bool { !no_screen || spec.output_file.is_some() }

/// 落地单组结果（对齐 GUI emit_namer_pf_result）：屏幕 `label metric:score`、
/// 文件 `score label`、技能榜 `title score label`。
fn emit_namer_pf_result(
    result: &NamerPfJobResult,
    metrics: &[&NamerPfMetricSpec],
    outputs: &mut [Option<fs::File>],
    skill_board: Option<&SkillBoardConfig>,
    skill_board_output: &mut Option<fs::File>,
    precision: usize,
    no_screen: bool,
) -> Result<(), String> {
    for (spec, output) in metrics.iter().zip(outputs.iter_mut()) {
        let score = result.scores.get(spec.metric);
        if !no_screen && spec.min_screen.is_none_or(|limit| score >= limit) {
            println!("{} {}:{}", result.label, spec.metric.label(), format_rate(score, precision));
        }
        if spec.min_file.is_none_or(|limit| score >= limit)
            && let Some(output) = output.as_mut()
        {
            writeln!(output, "{} {}", format_rate(score, precision), result.label)
                .map_err(|err| format!("写入输出文件失败: {err}"))?;
        }
    }
    if let Some(config) = skill_board {
        for line in evaluate_skill_board(&result.group, &result.scores, config) {
            let line_text = format!("{} {} {}", line.title, format_rate(line.score, precision), result.label);
            if !no_screen {
                println!("{line_text}");
            }
            if let Some(output) = skill_board_output.as_mut() {
                writeln!(output, "{line_text}").map_err(|err| format!("写入输出文件失败: {err}"))?;
            }
        }
    }
    Ok(())
}

/// 创建/截哑输出文件，语义与 GUI 的 create_output_file 一致。
fn create_output_file(path: &Path) -> Result<fs::File, String> {
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
    fs::File::create(path).map_err(|err| format!("打开输出文件失败: {}: {err}", path.display()))
}

/// 解析 `namer-pf` 每行一组、组内 `+` 分隔的输入。
fn parse_plus_separated_groups(raw: &str) -> Vec<Vec<String>> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_namer_pf_group_line)
        .filter(|group| !group.is_empty())
        .collect()
}

/// 解析 `namer-pf` 的单行组输入，并保留 overlay 后缀。
fn parse_namer_pf_group_line(line: &str) -> Vec<String> {
    let mut group: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut idx = 0usize;

    while idx < line.len() {
        let rest = &line[idx..];
        if rest.starts_with('+') {
            let after_plus = &line[idx + 1..];
            if let Some(overlay_end) = overlay_suffix_end(after_plus) {
                current.push('+');
                current.push_str(&after_plus[..overlay_end]);
                idx += 1 + overlay_end;
                continue;
            }

            push_namer_pf_segment(&mut group, &mut current);
            idx += 1;
            continue;
        }

        let ch = rest.chars().next().expect("rest should contain a char");
        current.push(ch);
        idx += ch.len_utf8();
    }
    push_namer_pf_segment(&mut group, &mut current);

    group
}

fn push_namer_pf_segment(group: &mut Vec<String>, current: &mut String) {
    let segment = current.trim();
    if !segment.is_empty() {
        group.push(segment.to_string());
    }
    current.clear();
}

/// 如果 `raw` 是紧跟在 `+` 后面的 overlay 后缀，返回该 overlay 的结束 byte index。
fn overlay_suffix_end(raw: &str) -> Option<usize> {
    if raw.starts_with("ol:") {
        let mut idx = 3usize;
        skip_ascii_ws(raw, &mut idx);
        return consume_balanced_ascii(raw, idx, b'{', b'}');
    }

    if raw.starts_with("diy[") {
        let mut idx = consume_balanced_ascii(raw, 3, b'[', b']')?;
        skip_ascii_ws(raw, &mut idx);
        if raw.as_bytes().get(idx).copied() == Some(b'{') {
            idx = consume_balanced_ascii(raw, idx, b'{', b'}')?;
        }
        return Some(idx);
    }

    None
}

fn skip_ascii_ws(raw: &str, idx: &mut usize) {
    let bytes = raw.as_bytes();
    while *idx < bytes.len() && bytes[*idx].is_ascii_whitespace() {
        *idx += 1;
    }
}

fn consume_balanced_ascii(raw: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = raw.as_bytes();
    if bytes.get(start).copied() != Some(open) {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut idx = start;

    while idx < bytes.len() {
        let byte = bytes[idx];
        if in_string {
            if escaped {
                escaped = false;
            } else {
                match byte {
                    b'\\' => escaped = true,
                    b'"' => in_string = false,
                    _ => {}
                }
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b if b == open => depth += 1,
                b if b == close => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(idx + 1);
                    }
                }
                _ => {}
            }
        }

        idx += 1;
    }

    None
}

/// 计算 `namer-pf` 四项中的一个分数。
fn namer_pf_score(
    base_group: &[String],
    modifier: &str,
    duplicate: bool,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
) -> f64 {
    let mut target_group = base_group.to_vec();
    if duplicate {
        target_group.extend(base_group.iter().cloned());
    }

    let summary = run_bench_score_inner(
        &target_group,
        modifier,
        n,
        mode,
        threads,
        eval_rq,
        ScoreBenchOptions {
            show_progress: false,
            timed: false,
        },
    );
    summary.wins as f64 * 10_000.0 / summary.total.max(1) as f64
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    /// 测试用临时目录，Drop 时清理。
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "tswn-cli-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn namer_pf_parser_accepts_plus_groups() {
        assert_eq!(
            parse_plus_separated_groups("aaaaa+bbbbb\nccccc\n\n"),
            vec![vec!["aaaaa".to_string(), "bbbbb".to_string()], vec!["ccccc".to_string()],]
        );
    }

    #[test]
    fn namer_pf_parser_keeps_diy_overlay_with_player() {
        let diy = r#"aaaaa+diy[58,87,82,78,89,93,99,343]{"skldefend":13,"sklassassinate":"2*46","sklheal":"40+30"}"#;
        let raw = format!("{diy}+bbbbb");

        assert_eq!(
            parse_plus_separated_groups(&raw),
            vec![vec![diy.to_string(), "bbbbb".to_string(),]]
        );
    }

    #[test]
    fn namer_pf_parser_keeps_ol_overlay_with_player() {
        let ol = r#"aaaaa+ol:{"attrs":[58,87,82,78,89,93,99,343],"skills":{"skldefend":13,"sklheal":"40+30"},"name_factor_enabled":true}"#;
        let raw = format!("{ol}+bbbbb");

        assert_eq!(
            parse_plus_separated_groups(&raw),
            vec![vec![ol.to_string(), "bbbbb".to_string(),]]
        );
    }

    #[test]
    fn namer_pf_parser_ignores_quotes_in_plain_player_name() {
        let diy = r#"J"*)uEx@Hell+ol:{"attrs":[98,88,97,89,92,93,93,351],"skills":{"skliron":"7+7","sklslow":"2*22"}}"#;
        let raw = format!("{diy}+bbbbb");

        assert_eq!(
            parse_plus_separated_groups(&raw),
            vec![vec![diy.to_string(), "bbbbb".to_string(),]]
        );
    }

    #[test]
    fn namer_pf_scores_get_maps_metric_to_score() {
        let scores = NamerPfScores {
            pp: 1.0,
            pd: 2.0,
            qp: 3.0,
            qd: 4.0,
            sum: 10.0,
        };

        assert_eq!(scores.get(NamerPfMetric::Pp), 1.0);
        assert_eq!(scores.get(NamerPfMetric::Pd), 2.0);
        assert_eq!(scores.get(NamerPfMetric::Qp), 3.0);
        assert_eq!(scores.get(NamerPfMetric::Qd), 4.0);
        assert_eq!(scores.get(NamerPfMetric::Sum), 10.0);
    }

    #[test]
    fn metric_emits_respects_no_screen_and_output_file() {
        let spec = NamerPfMetricSpec {
            metric: NamerPfMetric::Pp,
            min_screen: None,
            output_file: None,
            min_file: None,
        };
        let with_file = NamerPfMetricSpec {
            output_file: Some(PathBuf::from("out.txt")),
            ..spec.clone()
        };

        // 屏幕全局开启时一定有输出；--no-screen 后只有配了文件的指标才需要计算。
        assert!(metric_emits(&spec, false));
        assert!(!metric_emits(&spec, true));
        assert!(metric_emits(&with_file, false));
        assert!(metric_emits(&with_file, true));
    }

    #[test]
    fn create_output_file_rejects_path_without_file_name() {
        assert!(create_output_file(Path::new("")).is_err());
    }

    #[test]
    fn create_output_file_rejects_directory() {
        let dir = TempDir::new("namer-pf-out-dir");
        assert!(create_output_file(&dir.0).is_err());
    }

    #[test]
    fn create_output_file_creates_missing_parents() {
        let dir = TempDir::new("namer-pf-out-nested");
        let path = dir.0.join("nested").join("out.txt");

        let file = create_output_file(&path).expect("create nested output file");
        drop(file);

        assert!(path.is_file());
    }
}

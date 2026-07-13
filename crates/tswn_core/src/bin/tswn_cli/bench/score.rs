//! 评分 benchmark 与 `namer-pf` 的实现。
//!
//! 这一块的共同点是都依赖 JS 风格的 score profile 生成方式：
//! - 每一轮都会构造一份新的 profile 输入；
//! - 单人、双同名目标在 profile 数量上有特殊规则；
//! - 由于模板几乎不复用，必须避开全局缓存以免内存线性膨胀。

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use tswn_core::Runner;
use tswn_core::bench_sched::{low_accuracy_outer_workers, run_outer_parallel_ordered};
use tswn_core::player::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::runtime_v2::{RuntimeV2BatchSummary, runtime_v2_score, runtime_v2_score_range};
use tswn_core::win_rate::WinRateTiming;

use crate::args::{BenchThreadMode, NamerPfMode};

use super::common::{BenchSummary, thread_spec};
use super::output::{format_rate, print_perf_lines};

/// 显式带 modifier 的 score benchmark 入口。
pub(super) fn run_bench_score_with_modifier(
    groups: &[Vec<String>],
    modifier: &'static str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    perf: bool,
    buckets_step: Option<usize>,
) {
    let target_group = groups.first().cloned().unwrap_or_default();
    let target_count = target_group.len();
    if target_count == 0 {
        eprintln!("评分: 无目标玩家");
        return;
    }
    let label = if modifier == "!" { "!评分" } else { "普通评分" };

    println!("=== 实力评分测试 ({n} 场) ===");
    println!("目标: {}", target_group.join(", "));
    println!("info: {target_count}");

    let summary = if let Some(step) = buckets_step.filter(|step| *step > 0) {
        run_bench_score_with_bucket_output(&target_group, modifier, n, step)
    } else {
        run_bench_score_inner(&target_group, modifier, n, mode, threads, WIN_RATE_EVAL_RQ, true)
    };
    let score = summary.wins as f64 * 10_000.0 / summary.total.max(1) as f64;
    println!("{label}: {:.0} / 10000  ({}/{})", score, summary.wins, summary.total);
    if perf {
        print_perf_lines(summary.elapsed, summary.timing, summary.total);
    }
}

/// 标准 score benchmark 入口，同时输出普通评分与 `!评分`。
pub(super) fn run_bench_score(
    raw: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    perf: bool,
    buckets_step: Option<usize>,
) {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
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
        run_bench_score_with_bucket_output(&target_group, "\u{0002}", n, step)
    } else {
        run_bench_score_inner(&target_group, "\u{0002}", n, mode, threads, WIN_RATE_EVAL_RQ, true)
    };
    let ns = normal.wins as f64 * 10_000.0 / normal.total.max(1) as f64;
    println!("普通评分: {:.0} / 10000  ({}/{})", ns, normal.wins, normal.total);
    if perf {
        print_perf_lines(normal.elapsed, normal.timing, normal.total);
    }

    print!("[!评分]    ");
    let bang = if let Some(step) = buckets_step.filter(|step| *step > 0) {
        run_bench_score_with_bucket_output(&target_group, "!", n, step)
    } else {
        run_bench_score_inner(&target_group, "!", n, mode, threads, WIN_RATE_EVAL_RQ, true)
    };
    let bs = bang.wins as f64 * 10_000.0 / bang.total.max(1) as f64;
    println!("!评分:     {:.0} / 10000  ({}/{})", bs, bang.wins, bang.total);
    if perf {
        print_perf_lines(bang.elapsed, bang.timing, bang.total);
    }
}

/// 分段输出 score 累积结果。
fn run_bench_score_with_bucket_output(target_group: &[String], modifier: &str, n: usize, step: usize) -> BenchSummary {
    let started_at = Instant::now();
    let mut accumulated = RuntimeV2BatchSummary::default();
    let mut offset = 0usize;
    while offset < n {
        let chunk_end = (offset + step.max(1)).min(n);
        match runtime_v2_score_range(target_group, modifier, offset, chunk_end, WIN_RATE_EVAL_RQ) {
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

/// score benchmark 的统一执行器。
fn run_bench_score_inner(
    target_group: &[String],
    modifier: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
    show_progress: bool,
) -> BenchSummary {
    let started_at = Instant::now();
    let thread = match mode {
        BenchThreadMode::SingleThread => 1,
        BenchThreadMode::Parallel => thread_spec(threads),
    };
    let summary = match runtime_v2_score(target_group, modifier, n, eval_rq, thread) {
        Ok(summary) => summary,
        Err(error) => {
            eprintln!("执行 Runtime v2 评分失败: {error}");
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

/// `namer-pf` 入口。
pub fn run_namer_pf(raw: &str, n: usize, threads: Option<usize>, eval_rq: f64, precision: usize, modes: &[NamerPfMode]) {
    let groups = parse_plus_separated_groups(raw);
    if groups.is_empty() {
        eprintln!("namer-pf: 输入为空或无有效玩家");
        return;
    }
    let default_modes;
    let modes = if modes.is_empty() {
        default_modes = NamerPfMode::ALL;
        &default_modes[..]
    } else {
        modes
    };

    println!("{}", modes.iter().map(|mode| mode.label()).collect::<Vec<_>>().join("|"));

    // 低精度（1%/10%）档位且有多组输入时，外层按组并行、内层单线程，
    // 比让每组的 4 个 score 各自反复起线程更划算；其余情况维持原有内层并行。
    let outer_workers = low_accuracy_outer_workers(n, groups.len(), thread_spec(threads));
    if outer_workers > 1 {
        let cancel = AtomicBool::new(false);
        let _ = run_outer_parallel_ordered(
            &groups,
            outer_workers,
            &cancel,
            |_, group, _| namer_pf_line(group, modes, n, BenchThreadMode::SingleThread, threads, eval_rq, precision),
            || {},
            |line| {
                println!("{line}");
                Ok(())
            },
        );
    } else {
        for group in &groups {
            let line = namer_pf_line(group, modes, n, BenchThreadMode::Parallel, threads, eval_rq, precision);
            println!("{line}");
        }
    }
}

/// 计算并格式化单组 `namer-pf` 输出行（含末尾 sum）。
fn namer_pf_line(
    group: &[String],
    modes: &[NamerPfMode],
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
    precision: usize,
) -> String {
    let scores = modes
        .iter()
        .map(|m| {
            let (modifier, duplicate) = m.score_params();
            namer_pf_score(group, modifier, duplicate, n, mode, threads, eval_rq)
        })
        .collect::<Vec<_>>();
    let sum = scores.iter().sum::<f64>();
    scores
        .iter()
        .copied()
        .chain(std::iter::once(sum))
        .map(|score| format_rate(score, precision))
        .collect::<Vec<_>>()
        .join("|")
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

    let summary = run_bench_score_inner(&target_group, modifier, n, mode, threads, eval_rq, false);
    summary.wins as f64 * 10_000.0 / summary.total.max(1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

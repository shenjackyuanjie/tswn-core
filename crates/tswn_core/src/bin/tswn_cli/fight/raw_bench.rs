//! `raw` 子命令的普通对战与 benchmark 分流。

use tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::runtime::{RuntimeRunner, runtime_groups_win_rate, runtime_score};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawRoute {
    Fight,
    Benchmark,
}

fn raw_route(raw: &str) -> RawRoute {
    if starts_with_raw_bench_header(raw) {
        RawRoute::Benchmark
    } else {
        RawRoute::Fight
    }
}

pub fn run_raw(raw: String, n: usize, threads: Option<usize>) {
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        eprintln!("raw: 输入为空或无有效玩家");
        return;
    }

    if matches!(raw_route(&trimmed), RawRoute::Fight) {
        super::runtime::run_runtime_fight(trimmed, true);
        return;
    }

    let body = strip_raw_bench_header(&trimmed).trim().to_string();
    if body.is_empty() {
        eprintln!("raw: !test! 之后未提供有效输入");
        return;
    }

    let (groups, _) = RuntimeRunner::split_namerena_into_groups(body.clone());
    match groups.iter().filter(|group| !group.is_empty()).count() {
        0 => eprintln!("raw: !test! 之后未提供有效输入"),
        1 => run_raw_score(body, n, threads),
        2 => run_raw_winrate(body, n, threads),
        _ => eprintln!("raw: !test! 模式只支持 1 组（评分）或 2 组（胜率）输入"),
    }
}

fn run_raw_score(raw: String, n: usize, threads: Option<usize>) {
    let (groups, _) = RuntimeRunner::split_namerena_into_groups(raw);
    let target_group = groups.into_iter().next().unwrap_or_default();
    if target_group.is_empty() {
        eprintln!("评分: 无目标玩家");
        return;
    }

    println!("=== 原始 namerena 评分测试 ({n} 场) ===");
    println!("目标: {}", target_group.join(", "));
    println!("info: {}", target_group.len());

    print!("[普通评分] ");
    let normal = run_raw_score_inner(&target_group, "\u{0002}", n, threads);
    let score = normal.0 as f64 * 10_000.0 / normal.1.max(1) as f64;
    println!("普通评分: {:.0} / 10000  ({}/{})", score, normal.0, normal.1);

    print!("[!评分]    ");
    let bang = run_raw_score_inner(&target_group, "!", n, threads);
    let score = bang.0 as f64 * 10_000.0 / bang.1.max(1) as f64;
    println!("!评分:     {:.0} / 10000  ({}/{})", score, bang.0, bang.1);
}

fn run_raw_score_inner(target_group: &[String], modifier: &str, n: usize, threads: Option<usize>) -> (usize, usize) {
    let thread = threads.and_then(|value| u32::try_from(value).ok()).unwrap_or(0);
    match runtime_score(target_group, modifier, n, WIN_RATE_EVAL_RQ, thread) {
        Ok(summary) => (summary.wins, summary.total),
        Err(error) => {
            eprintln!("构建 Runtime 评分对局失败: {error}");
            (0, 0)
        }
    }
}

fn starts_with_raw_bench_header(raw: &str) -> bool {
    let raw = raw.trim_start_matches('\u{feff}');
    raw.strip_prefix("!test!")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
}

fn strip_raw_bench_header(raw: &str) -> &str { raw.trim_start_matches('\u{feff}').strip_prefix("!test!").unwrap_or(raw) }

fn run_raw_winrate(raw: String, n: usize, threads: Option<usize>) {
    println!("=== 原始 namerena 胜率测试 ({n} 场) ===");
    let summary = run_raw_winrate_inner(&raw, n, threads);
    let rate = summary.0 as f64 * 100.0 / summary.1.max(1) as f64;
    println!("胜率: {:.2}%  ({}/{})", rate, summary.0, summary.1);
}

fn run_raw_winrate_inner(raw: &str, n: usize, threads: Option<usize>) -> (usize, usize) {
    let (groups, _) = RuntimeRunner::split_namerena_into_groups(raw.to_owned());
    let thread = threads.and_then(|value| u32::try_from(value).ok()).unwrap_or(0);
    match runtime_groups_win_rate(&groups, n, WIN_RATE_EVAL_RQ, thread) {
        Ok(summary) => (summary.wins, summary.total),
        Err(error) => {
            eprintln!("构建胜率模板失败: {error}");
            (0, 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_bench_header_and_route_are_strict() {
        assert!(starts_with_raw_bench_header("!test!\n\nmario"));
        assert!(starts_with_raw_bench_header("\u{feff}!test!\n\nmario"));
        assert!(!starts_with_raw_bench_header("!test!mario"));
        assert_eq!(raw_route("mario\n\nluigi"), RawRoute::Fight);
        assert_eq!(raw_route("!test!\n\nmario"), RawRoute::Benchmark);
    }

    #[test]
    fn raw_bench_body_and_group_count_are_preserved() {
        let trimmed = "\n!test!\n\nmario\n\nluigi".trim();
        let body = strip_raw_bench_header(trimmed).trim();
        assert_eq!(body, "mario\n\nluigi");
        let (groups, _) = RuntimeRunner::split_namerena_into_groups(body.to_owned());
        assert_eq!(groups.iter().filter(|group| !group.is_empty()).count(), 2);
    }

    #[test]
    fn runtime_score_and_winrate_are_repeatable() {
        let target = vec!["mario".to_owned()];
        assert_eq!(
            run_raw_score_inner(&target, "!", 12, Some(1)),
            run_raw_score_inner(&target, "!", 12, Some(1))
        );
        let raw = "left@red\n\nright@blue";
        assert_eq!(run_raw_winrate_inner(raw, 24, Some(1)), run_raw_winrate_inner(raw, 24, Some(1)));
    }
}

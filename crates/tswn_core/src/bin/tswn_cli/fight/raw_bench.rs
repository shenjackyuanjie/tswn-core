//! `raw` 子命令里和 benchmark 相关的分流逻辑。
//!
//! `raw` 的特殊点在于：它既可能只是“把一场普通 fight 以 raw trace 打印出来”，
//! 也可能承载 `!test!` 开头的 benchmark 输入。
//!
//! 因此这里把“判断是不是 `!test!`”“评分路径如何构造 JS profile 输入”以及
//! “胜率 benchmark 怎么转交给底层实现”全部收拢到一起，避免普通 `fight` 入口沾上这些分支。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tswn_core::Runner;
use tswn_core::player::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::win_rate::groups_win_rate;

use crate::BENCH_PARALLEL_THRESHOLD;

use super::driver::{collect_input_player_ids, new_runner_from_raw_for_cli};
use super::trace::print_fight_raw;

/// 运行 `raw` 子命令。
///
/// 入口只做一个分派判断：
/// - 普通 raw 输入直接走 `print_fight_raw`；
/// - `!test!` 输入按组数分到评分或胜率 benchmark。
pub fn run_raw(raw: String, n: usize, threads: Option<usize>) {
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        eprintln!("raw: 输入为空或无有效玩家");
        return;
    }

    if !starts_with_raw_bench_header(&trimmed) {
        let mut runner = match new_runner_from_raw_for_cli(trimmed) {
            Ok(runner) => runner,
            Err(err) => {
                eprintln!("构建对局失败: {err}");
                std::process::exit(1);
            }
        };
        let input_player_ids = collect_input_player_ids(&runner);
        print_fight_raw(&mut runner, &input_player_ids);
        return;
    }

    let body = strip_raw_bench_header(&trimmed).trim().to_string();
    if body.is_empty() {
        eprintln!("raw: !test! 之后未提供有效输入");
        return;
    }

    let (groups, _) = Runner::split_namerena_into_groups(body.clone());
    let group_count = groups.iter().filter(|g| !g.is_empty()).count();
    match group_count {
        0 => eprintln!("raw: !test! 之后未提供有效输入"),
        1 => run_raw_score(body, n, threads),
        2 => run_raw_winrate(body, n, threads),
        _ => eprintln!("raw: !test! 模式只支持 1 组（评分）或 2 组（胜率）输入"),
    }
}

/// 运行 raw 评分 benchmark。
fn run_raw_score(raw: String, n: usize, threads: Option<usize>) {
    let (groups, _) = Runner::split_namerena_into_groups(raw.clone());
    let target_group = groups.into_iter().next().unwrap_or_default();
    let target_count = target_group.len();
    if target_count == 0 {
        eprintln!("评分: 无目标玩家");
        return;
    }

    println!("=== 原始 namerena 评分测试 ({n} 场) ===");
    println!("目标: {}", target_group.join(", "));
    println!("info: {target_count}");

    print!("[普通评分] ");
    let normal = run_raw_score_inner(&target_group, "\u{0002}", n, threads);
    let ns = normal.0 as f64 * 10_000.0 / normal.1.max(1) as f64;
    println!("普通评分: {:.0} / 10000  ({}/{})", ns, normal.0, normal.1);

    print!("[!评分]    ");
    let bang = run_raw_score_inner(&target_group, "!", n, threads);
    let bs = bang.0 as f64 * 10_000.0 / bang.1.max(1) as f64;
    println!("!评分:     {:.0} / 10000  ({}/{})", bs, bang.0, bang.1);
}

/// JS 评分路径里每轮真正需要追踪多少个目标位。
fn js_score_targets_per_round(target_group: &[String]) -> usize {
    if target_group.len() == 2 && target_group[0] == target_group[1] {
        1
    } else {
        target_group.len()
    }
}

/// JS 评分路径里每轮需要构造多少个 profile 对手。
fn js_score_profiles_per_round(target_group: &[String]) -> usize {
    if target_group.len() == 2 && target_group[0] == target_group[1] {
        1
    } else if target_group.len() == 1 {
        3
    } else {
        target_group.len()
    }
}

/// 生成一轮 JS 评分用的 namerena 输入。
///
/// 这里刻意直接往同一个 `bench_input` 缓冲区重写，避免在高场数 benchmark 下频繁分配字符串。
fn build_js_score_match_input(target_group: &[String], modifier: &str, round: usize, bench_input: &mut String) {
    bench_input.clear();

    let tracked_targets = js_score_targets_per_round(target_group);
    let profile_count = js_score_profiles_per_round(target_group);
    let profile_base = tswn_core::engine::PROFILE_START as usize + round * profile_count;

    if target_group.len() == 1 {
        bench_input.push_str(&target_group[0]);
        bench_input.push('\n');
        let _ = std::fmt::Write::write_fmt(bench_input, format_args!("{}@{modifier}", profile_base));
        bench_input.push_str("\n\n");
        let _ = std::fmt::Write::write_fmt(
            bench_input,
            format_args!("{}@{modifier}\n{}@{modifier}", profile_base + 1, profile_base + 2),
        );
        return;
    }

    for (idx, name) in target_group.iter().take(tracked_targets).enumerate() {
        if idx > 0 {
            bench_input.push('\n');
        }
        bench_input.push_str(name);
    }
    bench_input.push_str("\n\n");
    for offset in 0..profile_count {
        if offset > 0 {
            bench_input.push('\n');
        }
        let _ = std::fmt::Write::write_fmt(bench_input, format_args!("{}@{modifier}", profile_base + offset));
    }
}

/// raw score 的内层执行器。
fn run_raw_score_inner(target_group: &[String], modifier: &str, n: usize, threads: Option<usize>) -> (usize, usize) {
    let workers = resolve_raw_workers(threads, n);

    if workers <= 1 || n < BENCH_PARALLEL_THRESHOLD {
        return run_raw_score_range(target_group, modifier, 0, n, true);
    }

    let next = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let target_group = target_group.to_vec();
        let modifier = modifier.to_string();
        let next = Arc::clone(&next);
        handles.push(std::thread::spawn(move || {
            run_raw_score_worker(&target_group, modifier.as_str(), next.as_ref(), n)
        }));
    }

    let mut wins = 0usize;
    let mut total = 0usize;
    for handle in handles {
        let (part_wins, part_total) = handle.join().expect("raw score worker thread panicked");
        wins += part_wins;
        total += part_total;
    }
    (wins, total)
}

/// 单线程 raw score 执行区间。
fn run_raw_score_range(target_group: &[String], modifier: &str, start: usize, end: usize, show_progress: bool) -> (usize, usize) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut progress_printed = false;
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    for i in start..end {
        build_js_score_match_input(target_group, modifier, i, &mut bench_input);

        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        // `raw score` 的 profile 调度方式与 `bench score` 相同：每轮输入都会变化。
        // 若继续使用 cached Runner 构造，批量复盘时同样会把一次性模板堆进全局缓存。
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();

        runner.run_to_completion();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.iter().any(|winner| target_team.contains(winner))
        {
            wins += 1;
        }
        if show_progress && (i + 1) % 100 == 0 {
            print!("\r  进度: {}/{}  ", i + 1, end);
            progress_printed = true;
        }
    }
    if progress_printed {
        println!();
    }
    (wins, total)
}

/// 并行 raw score worker。
fn run_raw_score_worker(target_group: &[String], modifier: &str, next: &AtomicUsize, end: usize) -> (usize, usize) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= end {
            break;
        }

        build_js_score_match_input(target_group, modifier, i, &mut bench_input);

        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        // 并行 raw score 也必须跳过全局缓存；否则线程越多，只会越快把缓存填满。
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();

        runner.run_to_completion();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.iter().any(|winner| target_team.contains(winner))
        {
            wins += 1;
        }
    }

    (wins, total)
}

/// 解析 raw score 使用的 worker 数量。
fn resolve_raw_workers(threads: Option<usize>, total: usize) -> usize {
    threads
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|x| x.get().saturating_mul(5).div_ceil(4))
                .unwrap_or(1)
        })
        .min(total.max(1))
}

/// 判断输入是否以 `!test!` header 开始。
fn starts_with_raw_bench_header(raw: &str) -> bool {
    let raw = raw.trim_start_matches('\u{feff}');
    raw.strip_prefix("!test!")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
}

/// 去掉 `!test!` header，但保留后续原始 benchmark 主体。
fn strip_raw_bench_header(raw: &str) -> &str {
    let raw = raw.trim_start_matches('\u{feff}');
    raw.strip_prefix("!test!").unwrap_or(raw)
}

/// 运行 raw 胜率 benchmark。
fn run_raw_winrate(raw: String, n: usize, threads: Option<usize>) {
    println!("=== 原始 namerena 胜率测试 ({n} 场) ===");
    let summary = run_raw_winrate_inner(&raw, n, threads);
    let rate = summary.0 as f64 * 100.0 / summary.1.max(1) as f64;
    println!("胜率: {:.2}%  ({}/{})", rate, summary.0, summary.1);
}

/// raw 胜率 benchmark 的实际执行器。
fn run_raw_winrate_inner(raw: &str, n: usize, threads: Option<usize>) -> (usize, usize) {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let thread = threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0);
    match groups_win_rate(&groups, n, WIN_RATE_EVAL_RQ, thread) {
        Ok(summary) => (summary.wins, summary.total),
        Err(err) => {
            eprintln!("构建胜率模板失败: {err}");
            (0, 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_bench_header_detects_plain_header() {
        assert!(starts_with_raw_bench_header("!test!\n\nmario"));
    }

    #[test]
    fn raw_bench_header_detects_leading_blank_lines() {
        let trimmed = "\n\n!test!\n\nmario".trim().to_string();
        assert!(starts_with_raw_bench_header(&trimmed));
        assert_eq!(strip_raw_bench_header(&trimmed).trim(), "mario");
    }

    #[test]
    fn raw_bench_header_detects_bom_prefixed_header() {
        assert!(starts_with_raw_bench_header("\u{feff}!test!\n\nmario"));
    }

    #[test]
    fn raw_bench_header_rejects_non_header_inputs() {
        assert!(!starts_with_raw_bench_header("mario\n\nluigi"));
        assert!(!starts_with_raw_bench_header("!test!mario"));
    }

    #[test]
    fn strip_raw_bench_header_keeps_bench_body() {
        assert_eq!(strip_raw_bench_header("!test!\n\nmario\n\nluigi").trim(), "mario\n\nluigi");
    }

    #[test]
    fn trimmed_leading_blank_line_input_routes_to_bench_group_count() {
        let trimmed = "\n!test!\n\nmario\n\nluigi".trim().to_string();
        assert!(starts_with_raw_bench_header(&trimmed));

        let body = strip_raw_bench_header(&trimmed).trim().to_string();
        let (groups, _) = Runner::split_namerena_into_groups(body);
        let group_count = groups.iter().filter(|g| !g.is_empty()).count();

        assert_eq!(group_count, 2);
    }

    #[test]
    fn raw_score_single_target_builds_js_match_shape() {
        let single = ["aaaaa".to_string()];
        let mut bench_input = String::new();
        build_js_score_match_input(&single, "!", 0, &mut bench_input);
        assert_eq!(js_score_targets_per_round(&single), 1);
        assert_eq!(js_score_profiles_per_round(&single), 3);
        assert_eq!(bench_input, "aaaaa\n33554431@!\n\n33554432@!\n33554433@!");
    }
}
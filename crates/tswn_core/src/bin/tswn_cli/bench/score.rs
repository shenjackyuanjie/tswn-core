//! 评分 benchmark 与 `namer-pf` 的实现。
//!
//! 这一块的共同点是都依赖 JS 风格的 score profile 生成方式：
//! - 每一轮都会构造一份新的 profile 输入；
//! - 单人、双同名目标在 profile 数量上有特殊规则；
//! - 由于模板几乎不复用，必须避开全局缓存以免内存线性膨胀。

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use tswn_core::Runner;
use tswn_core::player::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::player::overlay::PlayerOverlay;
use tswn_core::win_rate::{WinRateTiming, resolve_win_rate_workers};

use crate::{BENCH_PARALLEL_THRESHOLD, args::BenchThreadMode};

use super::common::BenchSummary;
use super::output::print_perf_lines;

/// 根据线程模式和总任务量计算评分 benchmark worker 数。
fn resolve_bench_workers(mode: BenchThreadMode, threads: Option<usize>, total: usize) -> usize {
    match mode {
        BenchThreadMode::SingleThread => 1,
        BenchThreadMode::Parallel => resolve_win_rate_workers(threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0), total),
    }
}

/// JS score 路径里每轮真正需要追踪多少个 target。
fn js_score_targets_per_round(target_group: &[String]) -> usize {
    if target_group.len() == 2 && target_group[0] == target_group[1] {
        1
    } else {
        target_group.len()
    }
}

/// JS score 路径里每轮需要构造多少个 profile。
fn js_score_profiles_per_round(target_group: &[String]) -> usize {
    if target_group.len() == 2 && target_group[0] == target_group[1] {
        1
    } else if target_group.len() == 1 {
        3
    } else {
        target_group.len()
    }
}

/// 构造一轮 JS score 所需的 namerena 输入。
fn build_js_score_match_input(target_group: &[String], modifier: &str, round: usize, bench_input: &mut String) {
    bench_input.clear();

    let tracked_targets = js_score_targets_per_round(target_group);
    let profile_count = js_score_profiles_per_round(target_group);
    let profile_base = tswn_core::engine::PROFILE_START as usize + round * profile_count;

    if target_group.len() == 1 {
        bench_input.push_str(&target_group[0]);
        bench_input.push('\n');
        let _ = write!(bench_input, "{}@{modifier}", profile_base);
        bench_input.push_str("\n\n");
        let _ = write!(bench_input, "{}@{modifier}\n{}@{modifier}", profile_base + 1, profile_base + 2);
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
        let _ = write!(bench_input, "{}@{modifier}", profile_base + offset);
    }
}

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
        run_bench_score_inner(&target_group, modifier, n, mode, threads, true)
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
        run_bench_score_inner(&target_group, "\u{0002}", n, mode, threads, true)
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
        run_bench_score_inner(&target_group, "!", n, mode, threads, true)
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
    let (wins, total, timing) = run_bench_score_range_with_bucket_output(target_group, modifier, 0, n, step);
    BenchSummary {
        wins,
        total,
        timing,
        elapsed: started_at.elapsed(),
    }
}

/// score 分段执行器。
fn run_bench_score_range_with_bucket_output(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    step: usize,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    for i in start..end {
        build_js_score_match_input(target_group, modifier, i, &mut bench_input);

        let t_init = Instant::now();
        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        // score 路径每轮都会把 round 编进 profile 名字里；从缓存视角看，这几乎等价于
        // “每一局都是一组全新的 players key”。继续走 cached 构造只会不断制造新缓存项，
        // 命中率极低，却会让内存随场数线性增长。
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();

        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.first().is_some_and(|winner| target_team.contains(winner))
        {
            wins += 1;
        }

        if total.is_multiple_of(step) || i + 1 == end {
            let score = wins as f64 * 10_000.0 / total.max(1) as f64;
            println!("评分(分段): {:.0} / 10000  ({wins}/{total})", score);
        }
    }

    (wins, total, timing)
}

/// score benchmark 的统一执行器。
fn run_bench_score_inner(
    target_group: &[String],
    modifier: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    show_progress: bool,
) -> BenchSummary {
    let workers = resolve_bench_workers(mode, threads, n);
    let started_at = Instant::now();

    let mut summary = if workers <= 1 || n < BENCH_PARALLEL_THRESHOLD {
        let (wins, total, timing) = run_bench_score_range(target_group, modifier, 0, n, show_progress);
        BenchSummary {
            wins,
            total,
            timing,
            elapsed: Default::default(),
        }
    } else {
        let next = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let target_group = target_group.to_vec();
            let modifier = modifier.to_string();
            let next = Arc::clone(&next);
            handles.push(std::thread::spawn(move || {
                run_bench_score_worker(&target_group, modifier.as_str(), next.as_ref(), n)
            }));
        }

        let mut merged = BenchSummary {
            wins: 0,
            total: 0,
            timing: WinRateTiming::default(),
            elapsed: Default::default(),
        };
        for handle in handles {
            let (wins, total, timing) = handle.join().expect("score worker thread panicked");
            merged.wins += wins;
            merged.total += total;
            merged.timing.merge(timing);
        }
        merged
    };

    summary.elapsed = started_at.elapsed();
    summary
}

/// `namer-pf` 入口。
pub fn run_namer_pf(raw: &str, n: usize, threads: Option<usize>) {
    let groups = parse_plus_separated_groups(raw);
    if groups.is_empty() {
        eprintln!("namer-pf: 输入为空或无有效玩家");
        return;
    }

    println!("pp|pd|qp|qd");
    for group in groups {
        let pp = namer_pf_score(&group, "\u{0002}", false, n, threads);
        let pd = namer_pf_score(&group, "\u{0002}", true, n, threads);
        let qp = namer_pf_score(&group, "!", false, n, threads);
        let qd = namer_pf_score(&group, "!", true, n, threads);
        let sum = pp + pd + qp + qd;
        println!("{pp}|{pd}|{qp}|{qd}|{sum}");
    }
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
    for segment in split_plus_outside_quotes(line) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if PlayerOverlay::parse_inline(segment).is_some()
            && let Some(previous) = group.last_mut()
        {
            previous.push('+');
            previous.push_str(segment);
            continue;
        }
        group.push(segment.to_string());
    }
    group
}

/// 在引号外按 `+` 分割字符串，用于保留 overlay JSON 内部的 `+`。
fn split_plus_outside_quotes(raw: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if in_string {
            current.push(ch);
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
        } else if ch == '+' {
            segments.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
            if ch == '"' {
                in_string = true;
            }
        }
    }
    segments.push(current);
    segments
}

/// 计算 `namer-pf` 四项中的一个分数。
fn namer_pf_score(base_group: &[String], modifier: &str, duplicate: bool, n: usize, threads: Option<usize>) -> u64 {
    let mut target_group = base_group.to_vec();
    if duplicate {
        target_group.extend(base_group.iter().cloned());
    }

    let summary = run_bench_score_inner(&target_group, modifier, n, BenchThreadMode::Parallel, threads, false);
    (summary.wins as f64 * 10_000.0 / summary.total.max(1) as f64).round() as u64
}

/// 单线程区间 score 执行器。
fn run_bench_score_range(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    show_progress: bool,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut progress_printed = false;
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    for i in start..end {
        build_js_score_match_input(target_group, modifier, i, &mut bench_input);

        let t_init = Instant::now();
        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        // 并行 worker 与单线程路径是同一个问题：profile 名字持续变化，缓存几乎不会命中。
        // 这里必须走 uncached，避免多个 worker 一起向全局缓存灌入只用一次的模板。
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();

        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.first().is_some_and(|winner| target_team.contains(winner))
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
    (wins, total, timing)
}

/// 并行 score worker。
fn run_bench_score_worker(
    target_group: &[String],
    modifier: &str,
    next: &AtomicUsize,
    end: usize,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= end {
            break;
        }
        build_js_score_match_input(target_group, modifier, i, &mut bench_input);

        let t_init = Instant::now();
        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();

        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.first().is_some_and(|winner| target_team.contains(winner))
        {
            wins += 1;
        }
    }
    (wins, total, timing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_single_target_score_match_like_js() {
        let single = ["aaaaa".to_string()];
        let mut bench_input = String::new();
        build_js_score_match_input(&single, "!", 0, &mut bench_input);
        assert_eq!(js_score_targets_per_round(&single), 1);
        assert_eq!(js_score_profiles_per_round(&single), 3);
        assert_eq!(bench_input, "aaaaa\n33554431@!\n\n33554432@!\n33554433@!");
    }

    #[test]
    fn collapses_duplicate_single_target_like_js() {
        let duplicate = ["aaaaa".to_string(), "aaaaa".to_string()];
        let mut bench_input = String::new();
        build_js_score_match_input(&duplicate, "!", 0, &mut bench_input);
        assert_eq!(js_score_targets_per_round(&duplicate), 1);
        assert_eq!(js_score_profiles_per_round(&duplicate), 1);
        assert_eq!(bench_input, "aaaaa\n\n33554431@!");
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
}
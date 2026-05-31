//! 胜率 benchmark 的入口与调度。
//!
//! 这一层处理的是“给定一份已经读好的输入，应该跑哪一种胜率相关 benchmark”：
//! - 自动模式如何根据输入组数分流；
//! - 普通 win-rate 与 group-win-rate 的打印方式；
//! - 分段 buckets 输出的累积统计。

use std::time::{Duration, Instant};

use tswn_core::Runner;
use tswn_core::player::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::win_rate::{WinRateTiming, prepared_win_rate, run_prepared_win_rate_range, use_js_profile_seed_schedule};

use crate::args::BenchThreadMode;

use super::common::BenchSummary;
use super::output::{display_group, print_perf_lines};
use super::score::{run_bench_score, run_bench_score_with_modifier};

/// benchmark 自动模式的解析结果。
#[derive(Debug)]
struct BenchmarkInput {
    groups: Vec<Vec<String>>,
    score_modifier: Option<&'static str>,
}

/// 解析 benchmark 输入，并识别 JS `!test!` score marker。
fn parse_benchmark_input(raw: &str) -> BenchmarkInput {
    let (mut groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let mut score_modifier = None;

    if groups.first().and_then(|group| group.first()).is_some_and(|name| name == "!test!") {
        let marker_group = groups.remove(0);
        score_modifier = Some(if marker_group.get(1).is_some_and(|name| name == "!") {
            "!"
        } else {
            "\u{0002}"
        });
    }

    BenchmarkInput { groups, score_modifier }
}

/// 把拆开的 groups 重新拼回 namerena raw。
fn groups_to_raw(groups: &[Vec<String>]) -> String {
    groups
        .iter()
        .filter(|group| !group.is_empty())
        .map(|group| group.join("\n"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// 自动 benchmark 入口。
pub fn run_benchmark(
    raw: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    perf: bool,
    buckets_step: Option<usize>,
) {
    let raw = raw.trim();
    let BenchmarkInput { groups, score_modifier } = parse_benchmark_input(raw);
    let group_count = groups.iter().filter(|g| !g.is_empty()).count();
    match group_count {
        0 => eprintln!("benchmark: 输入为空或无有效玩家"),
        1 => {
            if let Some(modifier) = score_modifier {
                run_bench_score_with_modifier(&groups, modifier, n, mode, threads, perf, buckets_step);
            } else {
                run_bench_score(&groups_to_raw(&groups), n, mode, threads, perf, buckets_step);
            }
        }
        _ => run_bench_winrate(&groups_to_raw(&groups), n, mode, threads, WIN_RATE_EVAL_RQ, perf, buckets_step),
    }
}

/// 普通胜率 benchmark 入口。
pub fn run_bench_winrate(
    raw: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
    perf: bool,
    buckets_step: Option<usize>,
) {
    println!("=== 对战胜率测试 ({n} 场) ===");
    print_bench_winrate_matchup(raw);

    if let Some(step) = buckets_step {
        let summary = bench_winrate_with_buckets(raw, n, step, eval_rq);
        print_bench_winrate_summary(summary, perf);
    } else {
        let summary = bench_winrate_summary(raw, n, mode, threads, eval_rq);
        print_bench_winrate_summary(summary, perf);
    }
}

fn print_bench_winrate_matchup(raw: &str) {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let groups: Vec<_> = groups.into_iter().filter(|group| !group.is_empty()).collect();

    match groups.as_slice() {
        [team1, team2] => {
            println!("队伍 1: {}", format_group(team1));
            println!("队伍 2: {}", format_group(team2));
        }
        _ => {
            for (index, group) in groups.iter().enumerate() {
                println!("队伍 {}: {}", index + 1, format_group(group));
            }
        }
    }
}

fn format_group(group: &[String]) -> String { display_group(&group.join("\n")) }

/// 目标组对多个对手组的逐组胜率测试。
pub fn run_bench_group_win_rate(
    target: &str,
    against: &[String],
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
    perf: bool,
) {
    println!("=== 对组列表胜率测试 ({n} 场) ===");
    println!("target: {}", display_group(target));

    let overall_started = Instant::now();
    let mut accumulated_rate = 0.0;
    let mut accumulated_wins = 0usize;
    let mut accumulated_total = 0usize;
    let mut accumulated_timing = WinRateTiming::default();

    for (index, opponent) in against.iter().enumerate() {
        println!();
        println!("[{}/{}] vs {}", index + 1, against.len(), display_group(opponent));
        let raw = format!("{target}\n\n{opponent}");
        let summary = bench_winrate_summary(&raw, n, mode, threads, eval_rq);
        println!("胜率: {:.2}%  ({}/{})", summary.win_rate_percent(), summary.wins, summary.total);
        if perf {
            print_perf_lines(summary.elapsed, summary.timing, summary.total);
        }
        accumulated_rate += summary.win_rate_percent();
        accumulated_wins += summary.wins;
        accumulated_total += summary.total;
        accumulated_timing.merge(summary.timing);
    }

    println!();
    println!("平均胜率: {:.2}%", accumulated_rate / against.len().max(1) as f64);
    println!(
        "汇总胜率: {:.2}%  ({}/{})",
        accumulated_wins as f64 * 100.0 / accumulated_total.max(1) as f64,
        accumulated_wins,
        accumulated_total
    );
    if perf {
        print_perf_lines(overall_started.elapsed(), accumulated_timing, accumulated_total);
    }
}

/// 普通 win-rate 的实际执行器。
pub fn bench_winrate_summary(
    raw: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
) -> BenchSummary {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    // 这里的模板只服务当前这一个 matchup；`batch-rate` / `bench pair` 的外层循环
    // 会不断传入新的 `raw`，几乎没有缓存复用价值。改走 uncached 后，当前 matchup
    // 跑完即可释放模板，不会把大量一次性对阵长期压在全局缓存里。
    let prepared = match Runner::prepare_groups_with_eval_rq_uncached(&groups, eval_rq) {
        Ok(prepared) => prepared,
        Err(err) => {
            eprintln!("构建胜率模板失败: {err}");
            return BenchSummary {
                wins: 0,
                total: 0,
                timing: WinRateTiming::default(),
                elapsed: Duration::default(),
            };
        }
    };
    let started_at = Instant::now();

    let thread = match mode {
        BenchThreadMode::SingleThread => 1,
        BenchThreadMode::Parallel => threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0),
    };
    let summary = match prepared_win_rate(&prepared, n, eval_rq, thread) {
        Ok(summary) => summary,
        Err(err) => {
            eprintln!("执行胜率测试失败: {err}");
            return BenchSummary {
                wins: 0,
                total: 0,
                timing: WinRateTiming::default(),
                elapsed: Duration::default(),
            };
        }
    };

    let mut summary = BenchSummary {
        wins: summary.wins,
        total: summary.total,
        timing: summary.timing,
        elapsed: Duration::default(),
    };

    summary.elapsed = started_at.elapsed();
    summary
}

/// 分段累积胜率测试。按 `step` 将 `n` 场分块，每块结束后输出一次累积胜率。
/// 强制单线程以保证顺序正确。
fn bench_winrate_with_buckets(raw: &str, n: usize, step: usize, eval_rq: f64) -> BenchSummary {
    let step = step.max(1);
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    // 分段输出和普通 win-rate 一样，只消费当前这一份模板；这里没有必要把模板写入
    // 全局缓存，否则批量分析多个输入时缓存会只增不减。
    let prepared = match Runner::prepare_groups_with_eval_rq_uncached(&groups, eval_rq) {
        Ok(prepared) => prepared,
        Err(err) => {
            eprintln!("构建胜率模板失败: {err}");
            return BenchSummary {
                wins: 0,
                total: 0,
                timing: WinRateTiming::default(),
                elapsed: Duration::default(),
            };
        }
    };

    let started_at = Instant::now();
    let use_profile_seed = use_js_profile_seed_schedule(eval_rq);
    let mut cumulative_wins = 0usize;
    let mut cumulative_total = 0usize;
    let mut cumulative_timing = WinRateTiming::default();

    let mut offset = 0usize;
    while offset < n {
        let chunk_end = (offset + step).min(n);
        let chunk = match run_prepared_win_rate_range(&prepared, offset, chunk_end, use_profile_seed) {
            Ok(chunk) => chunk,
            Err(err) => {
                eprintln!("分段 [{offset}, {chunk_end}) 胜率测试失败: {err}");
                break;
            }
        };
        cumulative_wins += chunk.wins;
        cumulative_total += chunk.total;
        cumulative_timing.merge(chunk.timing);
        println!(
            "胜率(分段): {:.2}%  ({}/{})",
            cumulative_wins as f64 * 100.0 / cumulative_total.max(1) as f64,
            cumulative_wins,
            cumulative_total,
        );
        offset = chunk_end;
    }

    let mut summary = BenchSummary {
        wins: cumulative_wins,
        total: cumulative_total,
        timing: cumulative_timing,
        elapsed: Duration::default(),
    };
    summary.elapsed = started_at.elapsed();
    summary
}

/// 打印统一风格的 win-rate 摘要。
fn print_bench_winrate_summary(summary: BenchSummary, perf: bool) {
    let elapsed_secs = summary.elapsed.as_secs_f64();
    let throughput = if elapsed_secs > 0.0 {
        summary.total as f64 / elapsed_secs
    } else {
        0.0
    };
    println!("胜率: {:.2}%  ({}/{})", summary.win_rate_percent(), summary.wins, summary.total);
    println!(
        "耗时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
        elapsed_secs,
        summary.elapsed.as_micros() as f64 / summary.total.max(1) as f64,
        throughput
    );
    if perf {
        print_perf_lines(summary.elapsed, summary.timing, summary.total);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_js_default_score_marker() {
        let parsed = parse_benchmark_input("!test!\n\naaaa\nbbbb");
        assert_eq!(parsed.score_modifier, Some("\u{0002}"));
        assert_eq!(parsed.groups, vec![vec!["aaaa".to_string(), "bbbb".to_string()]]);
    }

    #[test]
    fn parses_js_bang_score_marker() {
        let parsed = parse_benchmark_input("!test!\n!\n\naaaa\nbbbb");
        assert_eq!(parsed.score_modifier, Some("!"));
        assert_eq!(parsed.groups, vec![vec!["aaaa".to_string(), "bbbb".to_string()]]);
    }

    #[test]
    fn parses_js_win_rate_marker() {
        let parsed = parse_benchmark_input("!test!\n\naaaa\n\nbbbb@!");
        assert_eq!(parsed.score_modifier, Some("\u{0002}"));
        assert_eq!(parsed.groups, vec![vec!["aaaa".to_string()], vec!["bbbb@!".to_string()]]);
        assert_eq!(groups_to_raw(&parsed.groups), "aaaa\n\nbbbb@!");
    }

    #[test]
    fn leaves_non_marker_input_unchanged() {
        let parsed = parse_benchmark_input("aaaa\n\nbbbb@!");
        assert_eq!(parsed.score_modifier, None);
        assert_eq!(parsed.groups, vec![vec!["aaaa".to_string()], vec!["bbbb@!".to_string()]]);
    }
}

//! `bench pair` 的二人组评估入口。
//!
//! 在 batch-rate 之上再套一层 player × teammate 组合循环：每个组合先按靶子权重
//! 得到平均胜率，`--teammate-factored` 时再乘队友权重，然后按乘权后的分数降序
//! 取前 head 个求和。共享的单组平均胜率计算在 `batch_rate.rs`，进度条在
//! `progress.rs`，文件输出与排序在 `output.rs`。

use std::cell::RefCell;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use tswn_core::bench_sched::{low_accuracy_outer_workers, run_outer_parallel_ordered};
use tswn_core::win_rate::WinRateTiming;

use crate::args::{BenchThreadMode, PairDetailMode};

use super::batch_rate::bench_batch_rate_for_group;
use super::common::thread_spec;
use super::output::{
    BatchFileOutputMode, ScoreOutputOptions, clean_name_label, finalize_score_output, format_batch_rate_log_record,
    format_batch_rate_pure_record, format_pair_rate_record, format_rate, open_batch_rate_output, player_to_ol_or_exit,
    print_perf_lines, write_batch_rate_record,
};
use super::progress::BatchProgress;

fn player_group_to_ol_or_exit(group: &str) -> String { group.lines().map(player_to_ol_or_exit).collect::<Vec<_>>().join("\n") }

/// 队友权重应用，对齐 openbox `teammate_score` 语义。
///
/// 先用靶子权重得到该队友组合的平均胜率，`teammate_factored` 时再乘队友权重；
/// 乘权后的分数才参与排序和 head 求和，因此队友权重会影响排名与最终分数，
/// 而不是只影响展示的平均胜率。
fn teammate_weighted_rate(average_rate: f64, teammate_factored: bool, teammate_factor: f64) -> f64 {
    if teammate_factored {
        average_rate * teammate_factor
    } else {
        average_rate
    }
}

/// 汇总一名选手的 pair 结果：按分数降序取前 `head` 个求和。
///
/// `pair_rates` 需已按 openbox 语义入队（乘权后的 cqp 与队友标签）；
/// 与 openbox `run_pair` 一致，`selected_count = min(head, 有效组合数)`。
fn summarize_pair_rates(pair_rates: &mut [(f64, String)], head: usize) -> (usize, f64) {
    pair_rates.sort_by(|a, b| b.0.total_cmp(&a.0));
    let selected_count = head.min(pair_rates.len());
    (
        selected_count,
        pair_rates.iter().take(selected_count).map(|(rate, _)| *rate).sum(),
    )
}

/// 生成 openbox 块状 cqp 详情，对齐 `format_pair_screen_log`。
///
/// 第一行是 `最终分数 名字`，随后逐行缩进两个空格输出 `  cqp 队友名字`。
/// `Top` 取前 `selected_count` 个（此时忽略 `detail_min`）；`Every` 输出所有
/// 不低于 `detail_min` 的组合，`detail_min` 为 `None` 时全输出。
/// `pair_rates` 需已按分数降序排列。
fn pair_detail_block(
    final_score: f64,
    player_label: &str,
    pair_rates: &[(f64, String)],
    detail: PairDetailMode,
    detail_min: Option<f64>,
    selected_count: usize,
    wr_precision: usize,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{} {}", format_rate(final_score, wr_precision), player_label);
    match detail {
        PairDetailMode::None => {}
        PairDetailMode::Top => {
            for (rate, teammate) in pair_rates.iter().take(selected_count) {
                let _ = writeln!(out, "  {} {}", format_rate(*rate, wr_precision), teammate);
            }
        }
        PairDetailMode::Every => {
            for (rate, teammate) in pair_rates {
                if detail_min.is_none_or(|limit| *rate >= limit) {
                    let _ = writeln!(out, "  {} {}", format_rate(*rate, wr_precision), teammate);
                }
            }
        }
    }
    out
}

/// pair 的队友权重与 cqp 详情配置（对齐 openbox 的带权队友预设与三模式详情）。
#[derive(Debug, Clone)]
pub struct PairOptions {
    /// 队友列表是否按带权 TOML 解析（与 `--target-factored` 同款格式）。
    pub teammate_factored: bool,
    /// 与队友列表对应的权重；普通文本队友全部为 `1.0`，长度与 `teammates` 一致。
    pub teammate_factors: Vec<f64>,
    /// cqp 详情模式，对齐 openbox GUI 的不显示 / 每组 / 有效三选一。
    pub detail: PairDetailMode,
    /// `detail = Every` 时的队友组合 cqp 阈值；其他模式下忽略，`None` 表示不过滤。
    pub detail_min: Option<f64>,
}

#[allow(clippy::too_many_arguments)]
pub fn run_bench_pair(
    target_groups: &[String],
    target_factors: &[f64],
    target_factored: bool,
    players: &[String],
    player_labels: &[String],
    teammates: &[String],
    teammate_labels: &[String],
    head: usize,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
    verbose: bool,
    perf: bool,
    out_file: Option<&Path>,
    force: bool,
    log: bool,
    pure: bool,
    min_screen: Option<f64>,
    min_file: Option<f64>,
    wr_precision: usize,
    options: PairOptions,
    output: ScoreOutputOptions,
) {
    let PairOptions {
        teammate_factored,
        teammate_factors,
        detail,
        detail_min,
    } = options;
    let ScoreOutputOptions { sort, clean_label } = output;
    let file_mode = if pure {
        BatchFileOutputMode::Pure
    } else if log {
        BatchFileOutputMode::Json
    } else {
        BatchFileOutputMode::Log
    };

    // 排序发生在所有选手处理完之后，先记住路径，避免被下面的 File 句柄遮蔽。
    let sort_path = out_file;

    let mut out_file = match out_file {
        Some(path) => match open_batch_rate_output(path, force) {
            Ok(file) => Some(file),
            Err(err) => {
                eprintln!("打开 pair 结果输出文件失败: {err}");
                std::process::exit(1);
            }
        },
        None => None,
    };

    println!(
        "=== 二人组 batch rate ({n} 场/对局, {} 选手, {} 队友, {} 靶子组, head={head}) ===",
        players.len(),
        teammates.len(),
        target_groups.len()
    );
    if let Some(threshold) = min_screen {
        println!("终端最低最终分数阈值: {}", format_rate(threshold, wr_precision));
    }
    if out_file.is_some()
        && let Some(threshold) = min_file
    {
        println!("文件最低最终分数阈值: {}", format_rate(threshold, wr_precision));
    }

    let total_matchups_per_player = teammates.len().saturating_mul(target_groups.len());
    let progress = RefCell::new(BatchProgress::new(players.len(), total_matchups_per_player));
    progress.borrow_mut().draw();
    let jobs = (0..players.len())
        .flat_map(|pi| (0..teammates.len()).map(move |ti| (pi, ti)))
        .collect::<Vec<_>>();
    let requested = if mode == BenchThreadMode::SingleThread {
        1
    } else {
        thread_spec(threads)
    };
    let workers = low_accuracy_outer_workers(n, jobs.len(), requested);
    let inner_threads = if workers > 1 { Some(1) } else { threads };
    let converted = players.iter().map(|player| player_group_to_ol_or_exit(player)).collect::<Vec<_>>();
    let cancel = AtomicBool::new(false);
    let mut pair_rates = Vec::new();
    let mut total_wins = 0;
    let mut total_battles = 0;
    let mut total_valid_matchups = 0;
    let mut total_skipped_matchups = 0;
    let mut total_timing = WinRateTiming::default();
    let mut verbose_buf = String::new();
    let mut overall_started = Instant::now();

    let result = run_outer_parallel_ordered(
        &jobs,
        workers,
        &cancel,
        |_, &(pi, ti), tick| {
            let started = Instant::now();
            let pair_group = format!("{}\n{}", converted[pi], teammates[ti]);
            let mut detail = String::new();
            if verbose {
                let _ = writeln!(detail, "  teammate: {}", teammates[ti]);
            }
            let summary = bench_batch_rate_for_group(
                &pair_group,
                target_groups,
                target_factors,
                target_factored,
                n,
                mode,
                inner_threads,
                eval_rq,
                verbose,
                &mut detail,
                |_, _| tick(),
            );
            if verbose {
                let _ = writeln!(
                    detail,
                    "  teammate avg: {}%  (有效 {}, 跳过 {})",
                    format_rate(summary.avg, wr_precision),
                    summary.valid_matchups,
                    summary.skipped_matchups
                );
            }
            (pi, ti, summary, detail, started)
        },
        || progress.borrow_mut().tick_target(),
        // detail_buf 是单个队友的 verbose 明细缓冲，避免与入参 detail（cqp 详情模式）混淆。
        |(pi, ti, summary, detail_buf, started)| {
            let player_label = &player_labels[pi];
            // 对齐 openbox：clean_label 时屏幕与文件标签统一剥掉 overlay 后缀。
            let display_player_label = if clean_label {
                clean_name_label(player_label)
            } else {
                player_label.to_string()
            };
            if ti == 0 {
                pair_rates.clear();
                total_wins = 0;
                total_battles = 0;
                total_valid_matchups = 0;
                total_skipped_matchups = 0;
                total_timing = WinRateTiming::default();
                verbose_buf.clear();
                overall_started = started;
                if verbose {
                    let _ = writeln!(
                        verbose_buf,
                        "\n━━━━━━━━ [{}/{}] {} ━━━━━━━━",
                        pi + 1,
                        players.len(),
                        display_player_label
                    );
                }
            }
            overall_started = overall_started.min(started);
            verbose_buf.push_str(&detail_buf);
            if summary.valid_matchups > 0 {
                // 对齐 openbox：先用靶子权重得到平均胜率（bench_batch_rate_for_group 已完成），
                // teammate_factored 时再乘队友权重，用乘权后的分数参与 head 排序与求和；
                // 队友权重影响排名与最终分数，不只是展示的平均胜率。
                let teammate_factor = teammate_factors.get(ti).copied().unwrap_or(1.0);
                pair_rates.push((
                    teammate_weighted_rate(summary.avg, teammate_factored, teammate_factor),
                    teammate_labels[ti].clone(),
                ));
            }
            total_wins += summary.wins;
            total_battles += summary.total;
            total_valid_matchups += summary.valid_matchups;
            total_skipped_matchups += summary.skipped_matchups;
            total_timing.merge(summary.timing);
            if ti + 1 != teammates.len() {
                return Ok(());
            }
            let (selected_count, final_score) = summarize_pair_rates(&mut pair_rates, head);
            // clean_label 时屏幕与文件输出统一使用清洗后的队友标签；顺序与 pair_rates 一致。
            let display_pair_rates = if clean_label {
                pair_rates
                    .iter()
                    .map(|(rate, teammate)| (*rate, clean_name_label(teammate)))
                    .collect::<Vec<_>>()
            } else {
                pair_rates.clone()
            };
            let elapsed = overall_started.elapsed();
            let elapsed_secs = elapsed.as_secs_f64();
            let throughput = if elapsed_secs > 0.0 {
                total_battles as f64 / elapsed_secs
            } else {
                0.0
            };
            let aggregate_rate = total_wins as f64 * 100.0 / total_battles.max(1) as f64;
            let summary_json = format_pair_rate_record(
                &display_player_label,
                final_score,
                selected_count,
                head,
                &display_pair_rates,
                aggregate_rate,
                total_wins,
                total_battles,
                elapsed,
                throughput,
                total_valid_matchups,
                total_skipped_matchups,
                wr_precision,
            );
            let summary_log = format_batch_rate_log_record(&display_player_label, final_score, wr_precision);
            let summary_pure = format_batch_rate_pure_record(&display_player_label);

            progress.borrow_mut().complete_player(elapsed);

            let passes_screen = min_screen.is_none_or(|t| final_score >= t);
            let passes_file = min_file.is_none_or(|t| final_score >= t);

            if passes_screen {
                progress.borrow_mut().clear();
                if verbose {
                    print!("{verbose_buf}");
                    println!("top {}:", selected_count);
                    for (index, (rate, teammate)) in display_pair_rates.iter().take(selected_count).enumerate() {
                        println!("  #{} {}% {}", index + 1, format_rate(*rate, wr_precision), teammate);
                    }
                    println!(
                        "最终分数: {}  (head={}, 有效组合 {}, 有效靶子 {}, 跳过 {} 场重复号)",
                        format_rate(final_score, wr_precision),
                        head,
                        pair_rates.len(),
                        total_valid_matchups,
                        total_skipped_matchups
                    );
                    println!(
                        "汇总胜率: {}%  ({}/{})",
                        format_rate(aggregate_rate, wr_precision),
                        total_wins,
                        total_battles
                    );
                    println!(
                        "用时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
                        elapsed_secs,
                        elapsed.as_micros() as f64 / total_battles.max(1) as f64,
                        throughput
                    );
                }
                // `--detail` 与 verbose 相互独立：块状 cqp 详情叠加在 verbose 输出之后；
                // 两者都不开启时保持原有 TSV 行格式。
                if detail == PairDetailMode::None {
                    if !verbose {
                        println!(
                            "{}\t最终分数: {}\ttop: {}/{}\t有效靶子: {}\t跳过重复: {}\t用时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
                            display_player_label,
                            format_rate(final_score, wr_precision),
                            selected_count,
                            head,
                            total_valid_matchups,
                            total_skipped_matchups,
                            elapsed_secs,
                            elapsed.as_micros() as f64 / total_battles.max(1) as f64,
                            throughput
                        );
                    }
                } else {
                    print!(
                        "{}",
                        pair_detail_block(
                            final_score,
                            &display_player_label,
                            &display_pair_rates,
                            detail,
                            detail_min,
                            selected_count,
                            wr_precision,
                        )
                    );
                }
            }

            if passes_file && let Some(file) = out_file.as_mut() {
                let line = match file_mode {
                    BatchFileOutputMode::Log => &summary_log,
                    BatchFileOutputMode::Json => &summary_json,
                    BatchFileOutputMode::Pure => &summary_pure,
                };
                if let Err(err) = write_batch_rate_record(file, line) {
                    eprintln!("写入 pair 结果输出文件失败: {err}");
                    std::process::exit(1);
                }
            }

            if perf && passes_screen {
                progress.borrow_mut().clear();
                print_perf_lines(elapsed, total_timing, total_battles);
            }

            progress.borrow_mut().draw();
            Ok(())
        },
    );
    if let Err(err) = result {
        eprintln!("pair 执行失败: {err}");
        std::process::exit(1);
    }

    progress.borrow_mut().finish();

    // 排序在输出文件关闭后进行，避免边写边重排；pure 模式由 helper 内部跳过。
    drop(out_file);
    if let Err(err) = finalize_score_output(sort_path, file_mode, sort) {
        eprintln!("排序 pair 输出文件失败: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use crate::args::{BenchThreadMode, PairDetailMode};

    #[test]
    fn multi_player_input_group_converts_each_member() {
        let group = "+ol:player-a\n+ol:player-b";
        assert_eq!(super::player_group_to_ol_or_exit(group), group);
    }

    /// 读取 pair 输出文件里的最终分数（Log 模式行首数字）。
    fn read_final_scores(path: &std::path::Path) -> Vec<f64> {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter_map(|line| line.split_whitespace().next().and_then(|token| token.parse().ok()))
            .collect()
    }

    /// 用镜像 50% 捷径跑一次 `run_bench_pair`：选手+队友与靶子组完全相同时记 50%，
    /// 不依赖真实对局的随机性，最终分数只由队友权重决定。
    fn run_pair_mirror_case(out_path: &std::path::Path, teammate_factored: bool, teammate_factors: &[f64]) {
        super::run_bench_pair(
            &["+ol:alpha\nmate@red".to_string()],
            &[1.0],
            true,
            &["+ol:alpha".to_string()],
            &["+ol:alpha".to_string()],
            &["mate@red".to_string()],
            &["mate@red".to_string()],
            1,
            1,
            BenchThreadMode::SingleThread,
            None,
            tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ,
            false,
            false,
            Some(out_path),
            true,
            false,
            false,
            None,
            None,
            3,
            super::PairOptions {
                teammate_factored,
                teammate_factors: teammate_factors.to_vec(),
                detail: PairDetailMode::None,
                detail_min: None,
            },
            super::ScoreOutputOptions {
                sort: false,
                clean_label: false,
            },
        );
    }

    #[test]
    fn teammate_factor_changes_head_selection_and_final_score() {
        // 仿 openbox tasks.rs 的 teammate_factor 语义：不乘权时 a(80) > b(60)，head=1
        // 只能取 a；先把胜率乘队友权重（a*0.5=40，b*2.0=120）再排序，head=1 改取 b。
        let mut plain = vec![(80.0, "a".to_string()), (60.0, "b".to_string())];
        assert_eq!(super::summarize_pair_rates(&mut plain, 1), (1, 80.0));

        let mut weighted = vec![
            (super::teammate_weighted_rate(80.0, true, 0.5), "a".to_string()),
            (super::teammate_weighted_rate(60.0, true, 2.0), "b".to_string()),
        ];
        assert_eq!(super::summarize_pair_rates(&mut weighted, 1), (1, 120.0));
        // head 超过有效组合数时全取，最终分数为全部乘权分数之和。
        assert_eq!(super::summarize_pair_rates(&mut weighted, 5), (2, 160.0));

        // 关闭 teammate_factored 时权重不参与计算，与 GUI 关闭 factor_enabled 一致。
        assert_eq!(super::teammate_weighted_rate(80.0, false, 0.5), 80.0);
    }

    #[test]
    fn pair_teammate_factor_scales_final_score_in_output_file() {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let plain_path = std::env::temp_dir().join(format!("tswn_pair_plain_{}_{}.txt", std::process::id(), stamp));
        let factored_path = std::env::temp_dir().join(format!("tswn_pair_factored_{}_{}.txt", std::process::id(), stamp));

        // 同一镜像对局复跑两次：唯一差别是 teammate_factored 与权重 2.0，
        // 最终分数应恰好放大一倍，证明队友权重走进了真实汇总路径。
        run_pair_mirror_case(&plain_path, false, &[1.0]);
        run_pair_mirror_case(&factored_path, true, &[2.0]);

        let plain = read_final_scores(&plain_path);
        let factored = read_final_scores(&factored_path);
        assert_eq!(plain.len(), 1);
        assert_eq!(factored.len(), 1);
        assert!(
            (factored[0] - 2.0 * plain[0]).abs() < 1e-6,
            "plain={plain:?}, factored={factored:?}"
        );

        let _ = std::fs::remove_file(&plain_path);
        let _ = std::fs::remove_file(&factored_path);
    }

    #[test]
    fn pair_detail_block_every_filters_and_top_truncates() {
        let rates = vec![(55.0, "a".to_string()), (45.0, "b".to_string()), (35.0, "c".to_string())];
        // Every + 阈值：只保留不低于 50 的组合。
        let every = super::pair_detail_block(55.0, "player", &rates, PairDetailMode::Every, Some(50.0), 3, 3);
        assert_eq!(every, "55.000 player\n  55.000 a\n");
        // Every 无阈值：全部输出。
        let all = super::pair_detail_block(55.0, "player", &rates, PairDetailMode::Every, None, 3, 3);
        assert_eq!(all.lines().count(), 4);
        // Top：截断到 selected_count，并忽略 detail_min。
        let top = super::pair_detail_block(100.0, "player", &rates, PairDetailMode::Top, Some(99.0), 2, 3);
        assert_eq!(top, "100.000 player\n  55.000 a\n  45.000 b\n");
        // None：只有首行（GUI 不显示 cqp 时不走块状格式，这里仅锁定 helper 行为）。
        let none = super::pair_detail_block(10.0, "player", &rates, PairDetailMode::None, None, 3, 3);
        assert_eq!(none, "10.000 player\n");
    }
}

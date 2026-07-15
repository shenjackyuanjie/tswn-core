//! CQP/CQD 矩阵的 Runtime v1/v2 同口径性能与结果对账工具。
//!
//! 输入读取、报告序列化和进程启动不计入墙钟；矩阵准备、worker 创建、matchup
//! 准备与全部战斗均计入。两套 runtime 使用完全相同的外层 worker 数。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use serde::Serialize;
use tswn_core::cli_api::parse_group_lines;
use tswn_core::player::Player;
use tswn_core::player::eval_name::DEFAULT_EVAL_RQ;
use tswn_core::runtime_v2::{RuntimeV2CqpMatchup, resolve_cqp_workers, runtime_v2_cqp_matchups};
use tswn_core::win_rate::groups_win_rate;

#[derive(Debug, Parser)]
#[command(
    name = "track_cqp_perf",
    about = "同口径测量并对账 CQP/CQD 的 Runtime v1 与 v2"
)]
struct Args {
    /// 每个非空行是一组选手的输入文件。
    #[arg(long, value_name = "FILE")]
    players: PathBuf,

    /// 每个非空行是一组靶子的输入文件。
    #[arg(long, value_name = "FILE")]
    targets: PathBuf,

    /// 报告中的输入标签。
    #[arg(long, default_value = "cqp-matrix")]
    label: String,

    /// 每个 matchup 执行的场数。
    #[arg(long, default_value_t = 100)]
    count: usize,

    /// 外层矩阵 worker 数；0 表示使用 Runtime v2 当前自动策略。
    #[arg(long, default_value_t = 0)]
    workers: usize,

    /// 要执行的 runtime。
    #[arg(long, value_enum, default_value_t = Engine::Both)]
    engine: Engine,

    /// 同时执行两个 runtime 时先跑哪一个。
    #[arg(long, value_enum, default_value_t = FirstEngine::V2)]
    first: FirstEngine,

    /// 选手组使用 `++` 分隔；默认使用 `+`。
    #[arg(long)]
    player_double_plus: bool,

    /// 靶子组使用 `++` 分隔；默认使用 `+`。
    #[arg(long)]
    target_double_plus: bool,

    /// 可选的 JSON 报告输出路径。
    #[arg(long, value_name = "FILE")]
    out: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Engine {
    V1,
    V2,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum FirstEngine {
    V1,
    V2,
}

#[derive(Clone, Debug)]
struct Matchup {
    player_index: usize,
    target_index: usize,
    groups: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Serialize)]
struct MatchupResult {
    player_index: usize,
    target_index: usize,
    wins: usize,
    total: usize,
    errors: usize,
}

#[derive(Debug, Serialize)]
struct EngineResult {
    engine: &'static str,
    elapsed_nanos: u128,
    wins: usize,
    total: usize,
    errors: usize,
    completed_matchups: usize,
    us_per_battle: f64,
    battles_per_second: f64,
    matchups: Vec<MatchupResult>,
}

#[derive(Debug, Serialize)]
struct MatchupMismatch {
    player_index: usize,
    target_index: usize,
    v1_wins: usize,
    v2_wins: usize,
    v1_total: usize,
    v2_total: usize,
    v1_errors: usize,
    v2_errors: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: u32,
    label: String,
    players: String,
    targets: String,
    count_per_matchup: usize,
    player_groups: usize,
    target_groups: usize,
    requested_matchups: usize,
    skipped_duplicate_matchups: usize,
    workers: usize,
    eval_rq: f64,
    timing_scope: &'static str,
    v1: Option<EngineResult>,
    v2: Option<EngineResult>,
    mismatch_count: usize,
    mismatches: Vec<MatchupMismatch>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("track_cqp_perf: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    if args.count == 0 {
        return Err("--count 必须大于 0".to_string());
    }

    let player_text =
        fs::read_to_string(&args.players).map_err(|error| format!("读取 {} 失败: {error}", args.players.display()))?;
    let target_text =
        fs::read_to_string(&args.targets).map_err(|error| format!("读取 {} 失败: {error}", args.targets.display()))?;
    let player_groups = parse_groups(&player_text, args.player_double_plus);
    let target_groups = parse_groups(&target_text, args.target_double_plus);
    if player_groups.is_empty() || target_groups.is_empty() {
        return Err("选手组和靶子组都不能为空".to_string());
    }

    let requested_matchups = player_groups.len() * target_groups.len();
    let (matchups, skipped_duplicate_matchups) = build_matchups(&player_groups, &target_groups);
    let workers = if args.workers == 0 {
        resolve_cqp_workers(0, matchups.len(), args.count)
    } else {
        args.workers.max(1).min(matchups.len().max(1))
    };

    let (v1, v2) = match args.engine {
        Engine::V1 => (Some(run_v1(&matchups, args.count, workers)), None),
        Engine::V2 => (None, Some(run_v2(&matchups, args.count, workers)?)),
        Engine::Both if args.first == FirstEngine::V1 => {
            let v1 = run_v1(&matchups, args.count, workers);
            let v2 = run_v2(&matchups, args.count, workers)?;
            (Some(v1), Some(v2))
        }
        Engine::Both => {
            let v2 = run_v2(&matchups, args.count, workers)?;
            let v1 = run_v1(&matchups, args.count, workers);
            (Some(v1), Some(v2))
        }
    };

    let mismatches = match (&v1, &v2) {
        (Some(v1), Some(v2)) => compare_results(&v1.matchups, &v2.matchups),
        _ => Vec::new(),
    };
    let report = Report {
        schema_version: 1,
        label: args.label,
        players: args.players.display().to_string(),
        targets: args.targets.display().to_string(),
        count_per_matchup: args.count,
        player_groups: player_groups.len(),
        target_groups: target_groups.len(),
        requested_matchups,
        skipped_duplicate_matchups,
        workers,
        eval_rq: DEFAULT_EVAL_RQ,
        timing_scope: "matrix wall; excludes build, process startup, input read and report serialization",
        v1,
        v2,
        mismatch_count: mismatches.len(),
        mismatches,
    };
    finish_report(report, args.out)
}

fn parse_groups(input: &str, double_plus: bool) -> Vec<Vec<String>> {
    parse_group_lines(input.trim_start_matches('\u{feff}'), double_plus)
        .into_iter()
        .map(|group| group.lines().map(str::to_owned).collect())
        .filter(|group: &Vec<String>| !group.is_empty())
        .collect()
}

fn build_matchups(players: &[Vec<String>], targets: &[Vec<String>]) -> (Vec<Matchup>, usize) {
    let mut matchups = Vec::with_capacity(players.len() * targets.len());
    let mut skipped = 0usize;
    for (player_index, player) in players.iter().enumerate() {
        for (target_index, target) in targets.iter().enumerate() {
            if has_duplicate_id_name(player, target) {
                skipped += 1;
                continue;
            }
            matchups.push(Matchup {
                player_index,
                target_index,
                groups: vec![player.clone(), target.clone()],
            });
        }
    }
    (matchups, skipped)
}

fn has_duplicate_id_name(left: &[String], right: &[String]) -> bool {
    let mut seen = std::collections::HashSet::new();
    left.iter().chain(right).any(|name| !seen.insert(Player::raw_namerena_to_idname(name)))
}

fn run_v1(matchups: &[Matchup], count: usize, workers: usize) -> EngineResult {
    let started = Instant::now();
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel();
    let mut ordered = (0..matchups.len()).map(|_| None).collect::<Vec<_>>();

    std::thread::scope(|scope| {
        for _ in 0..workers {
            let tx = tx.clone();
            let next = &next;
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= matchups.len() {
                        break;
                    }
                    let matchup = &matchups[index];
                    let result = match groups_win_rate(&matchup.groups, count, DEFAULT_EVAL_RQ, 1) {
                        Ok(summary) => MatchupResult {
                            player_index: matchup.player_index,
                            target_index: matchup.target_index,
                            wins: summary.wins,
                            total: summary.total,
                            errors: 0,
                        },
                        Err(_) => MatchupResult {
                            player_index: matchup.player_index,
                            target_index: matchup.target_index,
                            wins: 0,
                            total: 0,
                            errors: count,
                        },
                    };
                    if tx.send((index, result)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        while let Ok((index, result)) = rx.recv() {
            ordered[index] = Some(result);
        }
    });

    summarize_engine("v1", started.elapsed(), ordered.into_iter().flatten().collect())
}

fn run_v2(matchups: &[Matchup], count: usize, workers: usize) -> Result<EngineResult, String> {
    let requests = matchups
        .iter()
        .map(|matchup| RuntimeV2CqpMatchup::new(matchup.groups.clone()))
        .collect::<Vec<_>>();
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let batch = runtime_v2_cqp_matchups(&requests, count, DEFAULT_EVAL_RQ, workers as u32, &cancel, || {})
        .map_err(|error| format!("Runtime v2 矩阵执行失败: {error}"))?;
    let elapsed = started.elapsed();
    let mut results = Vec::with_capacity(matchups.len());
    for (matchup, outcome) in matchups.iter().zip(batch.matchups) {
        let result = match outcome {
            Some(outcome) => match outcome.summary {
                Ok(summary) => MatchupResult {
                    player_index: matchup.player_index,
                    target_index: matchup.target_index,
                    wins: summary.wins,
                    total: summary.total,
                    errors: summary.errors + summary.guard_exhausted,
                },
                Err(_) => MatchupResult {
                    player_index: matchup.player_index,
                    target_index: matchup.target_index,
                    wins: 0,
                    total: 0,
                    errors: count,
                },
            },
            None => MatchupResult {
                player_index: matchup.player_index,
                target_index: matchup.target_index,
                wins: 0,
                total: 0,
                errors: count,
            },
        };
        results.push(result);
    }
    Ok(summarize_engine("v2", elapsed, results))
}

fn summarize_engine(engine: &'static str, elapsed: Duration, matchups: Vec<MatchupResult>) -> EngineResult {
    let wins = matchups.iter().map(|result| result.wins).sum();
    let total = matchups.iter().map(|result| result.total).sum();
    let errors = matchups.iter().map(|result| result.errors).sum();
    let elapsed_nanos = elapsed.as_nanos();
    let elapsed_seconds = elapsed.as_secs_f64();
    EngineResult {
        engine,
        elapsed_nanos,
        wins,
        total,
        errors,
        completed_matchups: matchups.len(),
        us_per_battle: elapsed_nanos as f64 / 1_000.0 / total.max(1) as f64,
        battles_per_second: total as f64 / elapsed_seconds.max(f64::EPSILON),
        matchups,
    }
}

fn compare_results(v1: &[MatchupResult], v2: &[MatchupResult]) -> Vec<MatchupMismatch> {
    v1.iter()
        .zip(v2)
        .filter(|(v1, v2)| (v1.wins, v1.total, v1.errors) != (v2.wins, v2.total, v2.errors))
        .map(|(v1, v2)| MatchupMismatch {
            player_index: v1.player_index,
            target_index: v1.target_index,
            v1_wins: v1.wins,
            v2_wins: v2.wins,
            v1_total: v1.total,
            v2_total: v2.total,
            v1_errors: v1.errors,
            v2_errors: v2.errors,
        })
        .collect()
}

fn finish_report(report: Report, out: Option<PathBuf>) -> Result<(), String> {
    if let Some(v1) = &report.v1 {
        print_engine_summary(v1);
    }
    if let Some(v2) = &report.v2 {
        print_engine_summary(v2);
    }
    println!("对账差异: {}", report.mismatch_count);

    let mismatch_count = report.mismatch_count;
    let json = serde_json::to_string_pretty(&report).map_err(|error| format!("序列化报告失败: {error}"))?;
    if let Some(path) = out {
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|error| format!("创建 {} 失败: {error}", parent.display()))?;
        }
        fs::write(&path, format!("{json}\n")).map_err(|error| format!("写入 {} 失败: {error}", path.display()))?;
        println!("报告: {}", path.display());
    } else {
        println!("{json}");
    }
    if mismatch_count > 0 {
        return Err(format!("Runtime v1/v2 有 {mismatch_count} 个 matchup 结果不一致"));
    }
    Ok(())
}

fn print_engine_summary(result: &EngineResult) {
    println!(
        "{}: wall={:.6}s total={} errors={} {:.3}us/场 {:.2}场/s",
        result.engine,
        result.elapsed_nanos as f64 / 1_000_000_000.0,
        result.total,
        result.errors,
        result.us_per_battle,
        result.battles_per_second,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_and_plus_groups() {
        assert_eq!(
            parse_groups("mario\nluigi+peach\n", false),
            vec![vec!["mario".to_string()], vec!["luigi".to_string(), "peach".to_string()]]
        );
    }

    #[test]
    fn small_matrix_matches_between_runtimes() {
        let players = vec![vec!["left@red".to_string()]];
        let targets = vec![vec!["right@blue".to_string()]];
        let (matchups, skipped) = build_matchups(&players, &targets);
        assert_eq!(skipped, 0);
        let v1 = run_v1(&matchups, 24, 1);
        let v2 = run_v2(&matchups, 24, 1).unwrap();
        assert!(compare_results(&v1.matchups, &v2.matchups).is_empty());
    }
}

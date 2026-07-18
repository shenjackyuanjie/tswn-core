//! score 单线程裸性能与 Runtime/legacy 对账工具。
//!
//! 输入文件每个非空行是一组玩家，组内默认用 `+` 分隔。工具在开始计时前完成文件读取，
//! 执行期间不打印逐组进度；输出的 wall 因而只覆盖 core score 调用及其组内初始化/战斗。

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use serde::Serialize;
use tswn_core::LegacyRunner as Runner;
use tswn_core::cli_api::parse_group_lines;
use tswn_core::engine::PROFILE_START;
use tswn_core::player::eval_name::WIN_RATE_EVAL_RQ;
use tswn_core::runtime::{PreparedRuntimeRunner, default_custom_runtime_import_config, runtime_score};
use tswn_core::win_rate::WinRateTiming;

const MAX_ROUNDS: usize = 100_000;

#[derive(Debug, Parser)]
#[command(
    name = "track_score_perf",
    about = "测量 score 单线程裸时间，并对账 Runtime 与 legacy"
)]
struct Args {
    /// 每个非空行是一组玩家的输入文件。
    #[arg(long, value_name = "FILE")]
    input: PathBuf,

    /// 报告中使用的输入标签。
    #[arg(long, default_value = "score-cases")]
    label: String,

    /// 每一组执行的评分场数。
    #[arg(long, default_value_t = 1_000)]
    count: usize,

    /// 要执行的 runtime。
    #[arg(long, value_enum, default_value_t = Engine::Both)]
    engine: Engine,

    /// 同时执行两个 runtime 时先跑哪一个，用于交替消除顺序偏差。
    #[arg(long, value_enum, default_value_t = FirstEngine::Main)]
    first: FirstEngine,

    /// profile 评分模式。
    #[arg(long, value_enum, default_value_t = ScoreMode::Normal)]
    mode: ScoreMode,

    /// 组内使用 `++` 分隔；默认使用 `+`。
    #[arg(long)]
    double_plus: bool,

    /// 可选的 JSON 报告输出路径。
    #[arg(long, value_name = "FILE")]
    out: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Engine {
    Main,
    Legacy,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FirstEngine {
    Main,
    Legacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ScoreMode {
    Normal,
    Bang,
}

impl ScoreMode {
    fn modifier(self) -> &'static str {
        match self {
            Self::Normal => "\u{0002}",
            Self::Bang => "!",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Bang => "bang",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct GroupResult {
    index: usize,
    players: Vec<String>,
    wins: usize,
    total: usize,
    errors: usize,
    elapsed_nanos: u128,
    init_nanos: u128,
    fight_nanos: u128,
}

#[derive(Debug, Serialize)]
struct EngineResult {
    engine: &'static str,
    elapsed_nanos: u128,
    init_nanos: u128,
    fight_nanos: u128,
    wins: usize,
    total: usize,
    errors: usize,
    us_per_battle: f64,
    init_us_per_battle: f64,
    fight_us_per_battle: f64,
    battles_per_second: f64,
    groups: Vec<GroupResult>,
}

#[derive(Debug, Serialize)]
struct GroupMismatch {
    index: usize,
    players: Vec<String>,
    runtime_wins: usize,
    legacy_wins: usize,
    runtime_total: usize,
    legacy_total: usize,
    runtime_errors: usize,
    legacy_errors: usize,
    round_mismatches: Vec<RoundMismatch>,
}

#[derive(Debug, Serialize)]
struct RoundMismatch {
    round: usize,
    runtime_won: bool,
    legacy_won: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: u32,
    label: String,
    input: String,
    score_mode: &'static str,
    count_per_group: usize,
    group_count: usize,
    player_counts: Vec<usize>,
    thread: u32,
    timing_scope: &'static str,
    runtime: Option<EngineResult>,
    legacy: Option<EngineResult>,
    mismatch_count: usize,
    mismatches: Vec<GroupMismatch>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("track_score_perf: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    if args.count == 0 {
        return Err("--count 必须大于 0".to_string());
    }

    let input = fs::read_to_string(&args.input).map_err(|error| format!("读取 {} 失败: {error}", args.input.display()))?;
    let groups = parse_groups(&input, args.double_plus);
    if groups.is_empty() {
        return Err(format!("{} 没有有效输入组", args.input.display()));
    }

    let modifier = args.mode.modifier();
    let (runtime, legacy) = match args.engine {
        Engine::Main => (Some(run_runtime_batch(&groups, modifier, args.count)?), None),
        Engine::Legacy => (None, Some(run_legacy_batch(&groups, modifier, args.count))),
        Engine::Both if args.first == FirstEngine::Main => (
            Some(run_runtime_batch(&groups, modifier, args.count)?),
            Some(run_legacy_batch(&groups, modifier, args.count)),
        ),
        Engine::Both => {
            let legacy = run_legacy_batch(&groups, modifier, args.count);
            let runtime = run_runtime_batch(&groups, modifier, args.count)?;
            (Some(runtime), Some(legacy))
        }
    };

    let report = build_report(&args, &groups, runtime, legacy)?;
    finish_report(report, args.out)
}

fn parse_groups(input: &str, double_plus: bool) -> Vec<Vec<String>> {
    parse_group_lines(input.trim_start_matches('\u{feff}'), double_plus)
        .into_iter()
        .map(|raw| raw.lines().map(str::to_owned).collect::<Vec<_>>())
        .filter(|group| !group.is_empty())
        .collect()
}

fn run_runtime_batch(groups: &[Vec<String>], modifier: &str, count: usize) -> Result<EngineResult, String> {
    let started = Instant::now();
    let mut results = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let group_started = Instant::now();
        let summary = runtime_score(group, modifier, count, WIN_RATE_EVAL_RQ, 1)
            .map_err(|error| format!("Runtime 第 {} 组评分失败: {error}", index + 1))?;
        results.push(GroupResult {
            index,
            players: group.clone(),
            wins: summary.wins,
            total: summary.total,
            errors: summary.errors + summary.guard_exhausted,
            elapsed_nanos: group_started.elapsed().as_nanos(),
            init_nanos: summary.timing.init_nanos,
            fight_nanos: summary.timing.fight_nanos,
        });
    }
    Ok(summarize_engine("runtime", started.elapsed(), results))
}

fn run_legacy_batch(groups: &[Vec<String>], modifier: &str, count: usize) -> EngineResult {
    let started = Instant::now();
    let results = groups
        .iter()
        .enumerate()
        .map(|(index, group)| run_legacy_group(index, group, modifier, count))
        .collect();
    summarize_engine("legacy", started.elapsed(), results)
}

fn run_legacy_group(index: usize, target_group: &[String], modifier: &str, count: usize) -> GroupResult {
    let started = Instant::now();
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut errors = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    for round in 0..count {
        build_js_score_match_input(target_group, modifier, round, &mut bench_input);

        let init_started = Instant::now();
        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        let Ok(mut runner) = Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, WIN_RATE_EVAL_RQ) else {
            errors += 1;
            continue;
        };
        let target_team = runner.input_groups.first().cloned().unwrap_or_default();
        timing.init_nanos += init_started.elapsed().as_nanos();

        let fight_started = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += fight_started.elapsed().as_nanos();
        total += 1;
        if runner
            .world
            .winner
            .as_ref()
            .and_then(|winners| winners.first())
            .is_some_and(|winner| target_team.contains(winner))
        {
            wins += 1;
        }
    }

    GroupResult {
        index,
        players: target_group.to_vec(),
        wins,
        total,
        errors,
        elapsed_nanos: started.elapsed().as_nanos(),
        init_nanos: timing.init_nanos,
        fight_nanos: timing.fight_nanos,
    }
}

fn build_js_score_match_input(target_group: &[String], modifier: &str, round: usize, output: &mut String) {
    output.clear();
    let tracked_targets = js_score_targets_per_round(target_group);
    let profile_count = js_score_profiles_per_round(target_group);
    let profile_base = PROFILE_START as usize + round * profile_count;

    if target_group.len() == 1 {
        output.push_str(&target_group[0]);
        output.push('\n');
        let _ = write!(output, "{profile_base}@{modifier}");
        output.push_str("\n\n");
        let _ = write!(output, "{}@{modifier}\n{}@{modifier}", profile_base + 1, profile_base + 2);
        return;
    }

    for (index, name) in target_group.iter().take(tracked_targets).enumerate() {
        if index > 0 {
            output.push('\n');
        }
        output.push_str(name);
    }
    output.push_str("\n\n");
    for offset in 0..profile_count {
        if offset > 0 {
            output.push('\n');
        }
        let _ = write!(output, "{}@{modifier}", profile_base + offset);
    }
}

fn js_score_targets_per_round(target_group: &[String]) -> usize {
    if target_group.len() == 2 && target_group[0] == target_group[1] {
        1
    } else {
        target_group.len()
    }
}

fn js_score_profiles_per_round(target_group: &[String]) -> usize {
    if target_group.len() == 2 && target_group[0] == target_group[1] {
        1
    } else if target_group.len() == 1 {
        3
    } else {
        target_group.len()
    }
}

fn summarize_engine(engine: &'static str, elapsed: Duration, groups: Vec<GroupResult>) -> EngineResult {
    let wins: usize = groups.iter().map(|group| group.wins).sum();
    let total: usize = groups.iter().map(|group| group.total).sum();
    let errors: usize = groups.iter().map(|group| group.errors).sum();
    let init_nanos: u128 = groups.iter().map(|group| group.init_nanos).sum();
    let fight_nanos: u128 = groups.iter().map(|group| group.fight_nanos).sum();
    let elapsed_nanos = elapsed.as_nanos();
    let denominator = total.max(1) as f64;
    let elapsed_seconds = elapsed.as_secs_f64();

    EngineResult {
        engine,
        elapsed_nanos,
        init_nanos,
        fight_nanos,
        wins,
        total,
        errors,
        us_per_battle: elapsed_nanos as f64 / 1_000.0 / denominator,
        init_us_per_battle: init_nanos as f64 / 1_000.0 / denominator,
        fight_us_per_battle: fight_nanos as f64 / 1_000.0 / denominator,
        battles_per_second: total as f64 / elapsed_seconds.max(f64::EPSILON),
        groups,
    }
}

fn build_report(
    args: &Args,
    groups: &[Vec<String>],
    runtime: Option<EngineResult>,
    legacy: Option<EngineResult>,
) -> Result<Report, String> {
    let mismatches = match (&runtime, &legacy) {
        (Some(runtime), Some(legacy)) => compare_groups(&runtime.groups, &legacy.groups, args.mode.modifier(), args.count)?,
        _ => Vec::new(),
    };
    Ok(Report {
        schema_version: 1,
        label: args.label.clone(),
        input: args.input.display().to_string(),
        score_mode: args.mode.label(),
        count_per_group: args.count,
        group_count: groups.len(),
        player_counts: groups.iter().map(Vec::len).collect(),
        thread: 1,
        timing_scope: "core batch wall; excludes build, process startup, input read and report serialization",
        runtime,
        legacy,
        mismatch_count: mismatches.len(),
        mismatches,
    })
}

fn compare_groups(
    runtime: &[GroupResult],
    legacy: &[GroupResult],
    modifier: &str,
    count: usize,
) -> Result<Vec<GroupMismatch>, String> {
    runtime
        .iter()
        .zip(legacy)
        .filter(|(runtime, legacy)| (runtime.wins, runtime.total, runtime.errors) != (legacy.wins, legacy.total, legacy.errors))
        .map(|(runtime, legacy)| {
            Ok(GroupMismatch {
                index: runtime.index,
                players: runtime.players.clone(),
                runtime_wins: runtime.wins,
                legacy_wins: legacy.wins,
                runtime_total: runtime.total,
                legacy_total: legacy.total,
                runtime_errors: runtime.errors,
                legacy_errors: legacy.errors,
                round_mismatches: diagnose_round_mismatches(&runtime.players, modifier, count)?,
            })
        })
        .collect()
}

fn diagnose_round_mismatches(target_group: &[String], modifier: &str, count: usize) -> Result<Vec<RoundMismatch>, String> {
    let first_groups = score_match_groups(target_group, modifier, 0);
    let config = default_custom_runtime_import_config().map_err(|error| format!("构建 Runtime 默认配置失败: {error:?}"))?;
    let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&first_groups, WIN_RATE_EVAL_RQ, config)
        .map_err(|error| format!("构建 Runtime 评分诊断模板失败: {error:?}"))?;
    let mut runtime = prepared.new_reusable_runner();
    let mut mismatches = Vec::new();

    for round in 0..count {
        let groups = score_match_groups(target_group, modifier, round);
        let mut legacy = Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &[], WIN_RATE_EVAL_RQ)
            .map_err(|error| format!("legacy 第 {round} 场诊断初始化失败: {error:?}"))?;
        let target_team = legacy.input_groups.first().cloned().unwrap_or_default();
        legacy.run_to_completion();
        let legacy_won = legacy
            .world
            .winner
            .as_ref()
            .and_then(|winners| winners.first())
            .is_some_and(|winner| target_team.contains(winner));

        prepared
            .reset_from_groups_with_seed_and_eval_rq(&mut runtime, &groups, &[], WIN_RATE_EVAL_RQ)
            .map_err(|error| format!("Runtime 第 {round} 场诊断复位失败: {error:?}"))?;
        runtime.run_to_completion_prevalidated(MAX_ROUNDS);
        let runtime_won = runtime.input_group_won(0);
        if runtime_won != legacy_won {
            mismatches.push(RoundMismatch {
                round,
                runtime_won,
                legacy_won,
            });
        }
    }

    Ok(mismatches)
}

fn score_match_groups(target_group: &[String], modifier: &str, round: usize) -> Vec<Vec<String>> {
    let mut input = String::new();
    build_js_score_match_input(target_group, modifier, round, &mut input);
    Runner::split_namerena_into_groups(input).0
}

fn finish_report(report: Report, out: Option<PathBuf>) -> Result<(), String> {
    if let Some(runtime) = &report.runtime {
        print_engine_summary(runtime);
    }
    if let Some(legacy) = &report.legacy {
        print_engine_summary(legacy);
    }
    println!("对账差异: {}", report.mismatch_count);

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

    if report.mismatch_count > 0 {
        return Err(format!("Runtime/legacy 有 {} 组结果不一致", report.mismatch_count));
    }
    Ok(())
}

fn print_engine_summary(result: &EngineResult) {
    println!(
        "{}: wall={:.6}s init={:.6}s fight={:.6}s total={} errors={} {:.3}us/场 {:.2}场/s",
        result.engine,
        result.elapsed_nanos as f64 / 1_000_000_000.0,
        result.init_nanos as f64 / 1_000_000_000.0,
        result.fight_nanos as f64 / 1_000_000_000.0,
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
            vec![vec!["mario".to_string()], vec!["luigi".to_string(), "peach".to_string()],]
        );
    }

    #[test]
    fn builds_js_single_target_shape() {
        let mut output = String::new();
        build_js_score_match_input(&["mario".to_string()], "!", 2, &mut output);
        let base = PROFILE_START as usize + 6;
        assert_eq!(output, format!("mario\n{base}@!\n\n{}@!\n{}@!", base + 1, base + 2));
    }

    #[test]
    fn small_single_and_double_batches_match_legacy() {
        let groups = vec![vec!["mario".to_string()], vec!["luigi".to_string(), "peach".to_string()]];
        let runtime = run_runtime_batch(&groups, "\u{0002}", 8).unwrap();
        let legacy = run_legacy_batch(&groups, "\u{0002}", 8);
        assert!(compare_groups(&runtime.groups, &legacy.groups, "\u{0002}", 8).unwrap().is_empty());
    }
}

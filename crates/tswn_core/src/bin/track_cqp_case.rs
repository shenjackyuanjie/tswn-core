//! 单个 CQP/CQD matchup 的逐 seed legacy/v2 诊断工具。
//!
//! 本工具只在 `aux_bins` 下构建。正常 seed 只做无回放完成检查；发现胜负、guard
//! 或复用状态差异后，才重新构造 runner 做逐回合 strict diff。

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use clap::Parser;
use serde::Serialize;
use tswn_core::cli_api::parse_group_lines;
use tswn_core::player::eval_name::DEFAULT_EVAL_RQ;
use tswn_core::runtime_v2::{
    NormalizedOutcome, PreparedRuntimeV2Runner, RuntimeV2Runner, default_custom_runtime_v2_import_config, strict_diff,
};
use tswn_core::{PreparedRunner, Runner};

#[derive(Debug, Parser)]
#[command(
    name = "track_cqp_case",
    about = "逐 seed 定位单个 CQP/CQD matchup 的 legacy/v2 分叉与行动保护上限"
)]
struct Args {
    /// 每个非空行是一组选手的输入文件。
    #[arg(long, value_name = "FILE")]
    players: PathBuf,

    /// 每个非空行是一组靶子的输入文件。
    #[arg(long, value_name = "FILE")]
    targets: PathBuf,

    /// 选手组的零基下标。
    #[arg(long)]
    player_index: usize,

    /// 靶子组的零基下标。
    #[arg(long)]
    target_index: usize,

    /// 首个轮次编号，包含该轮。
    #[arg(long, default_value_t = 0)]
    start_round: usize,

    /// 末尾轮次编号，不包含该轮。
    #[arg(long, default_value_t = 10_000)]
    end_round: usize,

    /// 单场最多执行的可见行动轮数。
    #[arg(long, default_value_t = 100_000)]
    max_rounds: usize,

    /// 异常 seed 逐回合 strict diff 的最大轮数；0 表示不做 strict diff。
    #[arg(long, default_value_t = 10_000)]
    strict_limit: usize,

    /// 名字强度评估使用的 rq。
    #[arg(long, default_value_t = DEFAULT_EVAL_RQ)]
    eval_rq: f64,

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
struct CaseOutcome {
    won: bool,
    winner_team: Option<usize>,
    rounds: usize,
    guard_exhausted: bool,
    idle_exhausted: bool,
}

#[derive(Debug, Serialize)]
struct RoundAnomaly {
    round: usize,
    seed: Option<String>,
    legacy: CaseOutcome,
    v2_reused: CaseOutcome,
    v2_fresh: CaseOutcome,
    reused_matches_fresh: bool,
    first_strict_diff: Option<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: u32,
    players: String,
    targets: String,
    player_index: usize,
    target_index: usize,
    player_group: Vec<String>,
    target_group: Vec<String>,
    start_round: usize,
    end_round: usize,
    max_rounds: usize,
    strict_limit: usize,
    eval_rq: f64,
    anomaly_count: usize,
    anomalies: Vec<RoundAnomaly>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("track_cqp_case: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    if args.start_round >= args.end_round {
        return Err("--start-round 必须小于 --end-round".to_string());
    }
    if args.max_rounds == 0 {
        return Err("--max-rounds 必须大于 0".to_string());
    }
    if !args.eval_rq.is_finite() || args.eval_rq <= 0.0 {
        return Err("--eval-rq 必须是正数".to_string());
    }

    let player_text =
        fs::read_to_string(&args.players).map_err(|error| format!("读取 {} 失败: {error}", args.players.display()))?;
    let target_text =
        fs::read_to_string(&args.targets).map_err(|error| format!("读取 {} 失败: {error}", args.targets.display()))?;
    let player_groups = parse_groups(&player_text, args.player_double_plus);
    let target_groups = parse_groups(&target_text, args.target_double_plus);
    let player_group = player_groups
        .get(args.player_index)
        .ok_or_else(|| format!("player-index {} 越界，总组数 {}", args.player_index, player_groups.len()))?
        .clone();
    let target_group = target_groups
        .get(args.target_index)
        .ok_or_else(|| format!("target-index {} 越界，总组数 {}", args.target_index, target_groups.len()))?
        .clone();
    let groups = vec![player_group.clone(), target_group.clone()];

    let legacy_prepared = Runner::prepare_groups_with_eval_rq_uncached(&groups, args.eval_rq)
        .map_err(|error| format!("准备 legacy runner 失败: {error}"))?;
    let config = default_custom_runtime_v2_import_config().map_err(|error| format!("构造 Runtime v2 默认配置失败: {error:?}"))?;
    let v2_prepared = PreparedRuntimeV2Runner::from_custom_mixed_roster_with_eval_rq(&groups, args.eval_rq, config)
        .map_err(|error| format!("准备 Runtime v2 runner 失败: {error:?}"))?;
    let mut v2_reusable = v2_prepared.new_reusable_runner();
    let mut anomalies = Vec::new();
    let mut seed_buffer = String::with_capacity(24);

    for round in args.start_round..args.end_round {
        let seed = seed_for_round(&mut seed_buffer, round);
        let legacy = run_legacy(&legacy_prepared, seed, args.max_rounds)?;

        v2_prepared
            .reset_with_seed(&mut v2_reusable, seed)
            .map_err(|error| format!("round={round} 复位 Runtime v2 runner 失败: {error:?}"))?;
        let v2_reused = run_v2(&mut v2_reusable, args.max_rounds);

        if outcomes_match(legacy, v2_reused) {
            continue;
        }

        let mut v2_fresh_runner = v2_prepared
            .new_with_seed(seed)
            .map_err(|error| format!("round={round} 构造全新 Runtime v2 runner 失败: {error:?}"))?;
        let v2_fresh = run_v2(&mut v2_fresh_runner, args.max_rounds);
        let first_strict_diff = if args.strict_limit == 0 {
            None
        } else {
            Some(find_first_strict_diff(&legacy_prepared, &v2_prepared, seed, args.strict_limit)?)
        };
        let anomaly = RoundAnomaly {
            round,
            seed: seed.first().cloned(),
            legacy,
            v2_reused,
            v2_fresh,
            reused_matches_fresh: v2_reused == v2_fresh,
            first_strict_diff,
        };
        println!(
            "{}",
            serde_json::to_string(&anomaly).map_err(|error| format!("序列化异常失败: {error}"))?
        );
        anomalies.push(anomaly);
    }

    let report = Report {
        schema_version: 1,
        players: args.players.display().to_string(),
        targets: args.targets.display().to_string(),
        player_index: args.player_index,
        target_index: args.target_index,
        player_group,
        target_group,
        start_round: args.start_round,
        end_round: args.end_round,
        max_rounds: args.max_rounds,
        strict_limit: args.strict_limit,
        eval_rq: args.eval_rq,
        anomaly_count: anomalies.len(),
        anomalies,
    };
    finish_report(&report, args.out)
}

fn parse_groups(input: &str, double_plus: bool) -> Vec<Vec<String>> {
    parse_group_lines(input.trim_start_matches('\u{feff}'), double_plus)
        .into_iter()
        .map(|group| group.lines().map(str::to_owned).collect())
        .filter(|group: &Vec<String>| !group.is_empty())
        .collect()
}

fn seed_for_round(seed: &mut String, round: usize) -> &[String] {
    if round == 0 {
        &[]
    } else {
        seed.clear();
        let _ = write!(seed, "seed:{}@!", tswn_core::engine::PROFILE_START as usize + round);
        std::slice::from_ref(seed)
    }
}

fn run_legacy(prepared: &PreparedRunner, seed: &[String], max_rounds: usize) -> Result<CaseOutcome, String> {
    let mut runner =
        Runner::new_from_prepared_with_seed(prepared, seed).map_err(|error| format!("构造 legacy runner 失败: {error}"))?;
    let mut rounds = 0usize;
    let mut idle = 0usize;
    while !runner.have_winner() && rounds < max_rounds && idle <= 16 {
        let updates = runner.main_round();
        rounds += 1;
        if updates.had_updates() {
            idle = 0;
        } else {
            idle += 1;
        }
    }
    Ok(CaseOutcome {
        won: legacy_input_group_won(&runner, 0),
        winner_team: runner.winner_team_index(),
        rounds,
        guard_exhausted: !runner.have_winner() && rounds == max_rounds,
        idle_exhausted: !runner.have_winner() && idle > 16,
    })
}

fn run_v2(runner: &mut RuntimeV2Runner, max_rounds: usize) -> CaseOutcome {
    let completion = runner.run_to_completion_prevalidated(max_rounds);
    CaseOutcome {
        won: runner.input_group_won(0),
        winner_team: completion.winner_team,
        rounds: completion.rounds,
        guard_exhausted: completion.guard_exhausted,
        idle_exhausted: false,
    }
}

fn legacy_input_group_won(runner: &Runner, group_index: usize) -> bool {
    let Some(winners) = runner.world.winner.as_ref() else {
        return false;
    };
    runner
        .input_groups
        .get(group_index)
        .is_some_and(|group| winners.iter().any(|winner| group.contains(winner)))
}

fn outcomes_match(legacy: CaseOutcome, v2: CaseOutcome) -> bool {
    legacy.won == v2.won && legacy.guard_exhausted == v2.guard_exhausted && !legacy.idle_exhausted
}

fn find_first_strict_diff(
    legacy_prepared: &PreparedRunner,
    v2_prepared: &PreparedRuntimeV2Runner,
    seed: &[String],
    max_rounds: usize,
) -> Result<String, String> {
    let mut legacy = Runner::new_from_prepared_with_seed(legacy_prepared, seed)
        .map_err(|error| format!("strict diff 构造 legacy runner 失败: {error}"))?;
    let mut v2 = v2_prepared
        .new_with_seed(seed)
        .map_err(|error| format!("strict diff 构造 Runtime v2 runner 失败: {error:?}"))?;
    let mut first_any_diff = None;

    for index in 0..max_rounds {
        let legacy_finished = legacy.have_winner();
        let v2_finished = v2.runtime().world.winner_team().is_some();
        if legacy_finished || v2_finished {
            let legacy_won = legacy_input_group_won(&legacy, 0);
            let v2_won = v2.input_group_won(0);
            if legacy_finished == v2_finished && legacy_won == v2_won {
                return Ok(first_any_diff.unwrap_or_else(|| format!("前 {index} 轮完全一致并同时结束")));
            }
            let winner_diff = format!(
                "round={} 行动前胜负不同: legacy_finished={legacy_finished}, legacy_won={legacy_won}, \
                 v2_finished={v2_finished}, v2_won={v2_won}",
                index + 1,
            );
            return Ok(join_first_diff(first_any_diff, winner_diff));
        }

        let updates = legacy.main_round();
        let expected = NormalizedOutcome::from_legacy_runner(&legacy, index as u64 + 1, &updates);
        let actual = v2.run_round_normalized();
        if let Err(diff) = strict_diff(&expected, &actual) {
            first_any_diff.get_or_insert_with(|| format!("首个 strict diff: round={} {diff:?}", index + 1));
        }
        if let Some(diff) = first_non_score_diff(&expected, &actual) {
            return Ok(join_first_diff(
                first_any_diff,
                format!("首个非 score diff: round={} {diff}", index + 1),
            ));
        }
    }
    Ok(first_any_diff.unwrap_or_else(|| format!("前 {max_rounds} 轮未发现 strict diff")))
}

fn join_first_diff(first: Option<String>, next: String) -> String {
    first.map_or(next.clone(), |first| format!("{first}; {next}"))
}

fn first_non_score_diff(expected: &NormalizedOutcome, actual: &NormalizedOutcome) -> Option<String> {
    if expected.winner_team != actual.winner_team {
        return Some(format!(
            "winner expected={:?} actual={:?}",
            expected.winner_team, actual.winner_team
        ));
    }
    if expected.rng != actual.rng {
        return Some(format!("rng expected={:?} actual={:?}", expected.rng, actual.rng));
    }
    if expected.entity_ids != actual.entity_ids {
        return Some(format!(
            "entity_ids expected={:?} actual={:?}",
            expected.entity_ids, actual.entity_ids
        ));
    }
    if expected.teams != actual.teams {
        return Some(format!("teams expected={:?} actual={:?}", expected.teams, actual.teams));
    }
    if expected.hp != actual.hp {
        return Some(format!("hp expected={:?} actual={:?}", expected.hp, actual.hp));
    }
    if expected.magic_point != actual.magic_point {
        return Some(format!(
            "magic_point expected={:?} actual={:?}",
            expected.magic_point, actual.magic_point
        ));
    }
    if expected.defense != actual.defense {
        return Some(format!("defense expected={:?} actual={:?}", expected.defense, actual.defense));
    }
    if expected.resistance != actual.resistance {
        return Some(format!(
            "resistance expected={:?} actual={:?}",
            expected.resistance, actual.resistance
        ));
    }
    if expected.alive != actual.alive {
        return Some(format!("alive expected={:?} actual={:?}", expected.alive, actual.alive));
    }
    if expected.round_order != actual.round_order {
        return Some(format!(
            "round_order expected={:?} actual={:?}",
            expected.round_order, actual.round_order
        ));
    }
    if expected.flat_alive != actual.flat_alive {
        return Some(format!(
            "flat_alive expected={:?} actual={:?}",
            expected.flat_alive, actual.flat_alive
        ));
    }
    if expected.team_alive != actual.team_alive {
        return Some(format!(
            "team_alive expected={:?} actual={:?}",
            expected.team_alive, actual.team_alive
        ));
    }
    if expected.alive_group_count != actual.alive_group_count {
        return Some(format!(
            "alive_group_count expected={} actual={}",
            expected.alive_group_count, actual.alive_group_count
        ));
    }
    #[cfg(not(feature = "no_debug"))]
    if expected.actions != actual.actions {
        return Some(format!("actions expected={:?} actual={:?}", expected.actions, actual.actions));
    }
    if expected.frames.len() != actual.frames.len() {
        return Some(format!(
            "frame_count expected={} actual={}",
            expected.frames.len(),
            actual.frames.len()
        ));
    }
    for (index, (expected, actual)) in expected.frames.iter().zip(&actual.frames).enumerate() {
        let same_except_score = expected.message == actual.message
            && expected.caster == actual.caster
            && expected.target == actual.target
            && expected.targets == actual.targets
            && expected.param == actual.param
            && expected.delay0 == actual.delay0
            && expected.delay1 == actual.delay1
            && expected.update_type == actual.update_type;
        if !same_except_score {
            return Some(format!("frame[{index}] expected={expected:?} actual={actual:?}"));
        }
    }
    None
}

fn finish_report(report: &Report, out: Option<PathBuf>) -> Result<(), String> {
    println!("异常 seed 数: {}", report.anomaly_count);
    if let Some(path) = out {
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|error| format!("创建 {} 失败: {error}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(report).map_err(|error| format!("序列化报告失败: {error}"))?;
        fs::write(&path, format!("{json}\n")).map_err(|error| format!("写入 {} 失败: {error}", path.display()))?;
        println!("报告: {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_groups_respects_single_and_double_plus_separators() {
        let single = parse_groups("甲+乙\n丙+丁\n", false);
        assert_eq!(
            single,
            vec![
                vec!["甲".to_string(), "乙".to_string()],
                vec!["丙".to_string(), "丁".to_string()]
            ]
        );

        let double = parse_groups("甲++乙\n丙++丁\n", true);
        assert_eq!(
            double,
            vec![
                vec!["甲".to_string(), "乙".to_string()],
                vec!["丙".to_string(), "丁".to_string()]
            ]
        );
    }

    #[test]
    fn seed_schedule_matches_cqp_round_numbering() {
        let mut seed = String::new();
        assert!(seed_for_round(&mut seed, 0).is_empty());
        assert_eq!(seed_for_round(&mut seed, 135), &["seed:33554566@!".to_string()]);
    }

    #[test]
    fn outcome_comparison_uses_input_group_result_instead_of_raw_team_id() {
        let legacy = CaseOutcome {
            won: true,
            winner_team: Some(0),
            rounds: 10,
            guard_exhausted: false,
            idle_exhausted: false,
        };
        let reordered_v2 = CaseOutcome {
            won: true,
            winner_team: Some(1),
            rounds: 12,
            guard_exhausted: false,
            idle_exhausted: false,
        };
        assert!(outcomes_match(legacy, reordered_v2));

        let guarded_v2 = CaseOutcome {
            guard_exhausted: true,
            ..reordered_v2
        };
        assert!(!outcomes_match(legacy, guarded_v2));
    }
}

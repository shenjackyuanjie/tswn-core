//! CLI 侧的 Runtime v2 正式入口与迁移/调试入口。

use std::collections::HashMap;

use super::driver::fmt_runtime_v2_winner_input_indices;
use super::trace::{collect_runtime_v2_diff_lines, collect_runtime_v2_fight_raw_lines, fmt_runtime_v2_update};

use tswn_core::cli_api::{self as core_cli_api, CliApiError, JsonRuntimeV2NormalizedRun, JsonRuntimeV2ParityReport};
use tswn_core::engine::update::UpdateType;
use tswn_core::runtime_v2::{EntityIdx, RuntimeV2Runner};

pub(super) fn run_runtime_v2_fight(raw: String, out_raw: bool) {
    let mut runner = match core_cli_api::default_custom_runtime_v2_mixed_runner(&raw).map_err(cli_api_error) {
        Ok(runner) => runner,
        Err(err) => {
            eprintln!("构建对局失败: {err}");
            std::process::exit(1);
        }
    };
    let input_player_count = runner.runtime().entities.len();
    let lines = if out_raw {
        collect_runtime_v2_fight_raw_lines(&mut runner, input_player_count)
    } else {
        collect_runtime_v2_fight_lines(&mut runner, input_player_count, 100_000)
    };
    if !lines.is_empty() {
        println!("{}", lines.join("\n"));
    }
}

pub(super) fn run_runtime_v2_diff(raw: String) {
    match runtime_v2_diff_lines(&raw, 20_000) {
        Ok(lines) => {
            if !lines.is_empty() {
                println!("{}", lines.join("\n"));
            }
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

pub fn run_runtime_v2_normalized(raw: String, max_rounds: usize) {
    match runtime_v2_normalized_json(&raw, max_rounds) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

pub fn run_runtime_v2_parity(raw: String, max_rounds: usize) {
    match runtime_v2_parity_json(&raw, max_rounds) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn runtime_v2_diff_lines(raw: &str, max_rounds: usize) -> Result<Vec<String>, String> {
    let mut runner = core_cli_api::default_custom_runtime_v2_mixed_runner(raw).map_err(cli_api_error)?;
    let (lines, _guard, _total_score) = collect_runtime_v2_diff_lines(&mut runner, max_rounds, true);
    Ok(lines)
}

fn runtime_v2_display_stats(runner: &RuntimeV2Runner, entity_idx: EntityIdx) -> Option<String> {
    let entity = runner.runtime().entities.get(entity_idx)?;
    let all_sum = entity.template.clone_build.as_ref().map_or(
        entity
            .template
            .attr_sum
            .saturating_mul(3)
            .saturating_add(entity.template.max_hp.max(0) as u32),
        |build| build.all_sum(),
    );
    let name_factor = entity.template.clone_build.as_ref().map_or(0.0, |build| build.name_factor());
    Some(format!(
        "- {} (id={}): HP={}/{}, move_point:{} ATK={}, DEF={}, SPD={}, AGI={}, MAG={}, MP={}, MDF={}, ITL={}, all_sum={} 系数: {}",
        entity.template.display_name,
        entity_idx.0,
        entity.runtime.hp,
        entity.template.max_hp,
        entity.runtime.move_state.speed_points,
        entity.runtime.attack,
        entity.runtime.defense,
        entity.runtime.speed,
        entity.runtime.agility,
        entity.runtime.magic,
        entity.runtime.magic_point,
        entity.runtime.resistance,
        entity.runtime.wisdom,
        all_sum,
        name_factor,
    ))
}

fn collect_runtime_v2_fight_lines(runner: &mut RuntimeV2Runner, input_player_count: usize, max_rounds: usize) -> Vec<String> {
    let mut lines = vec!["=== 玩家状态 ===".to_owned()];
    for entity_idx in 0..input_player_count {
        let entity_idx = EntityIdx(entity_idx.try_into().expect("runtime v2 CLI input entity index overflow"));
        if let Some(line) = runtime_v2_display_stats(runner, entity_idx) {
            lines.push(line);
        }
    }
    lines.push(String::new());

    let mut round = 1usize;
    let mut idle_rounds = 0usize;
    let mut total_score = 0u64;
    let mut score_by_caster = HashMap::<usize, u64>::new();
    while runner.runtime().world.winner_team().is_none() && round <= max_rounds {
        let outcome = runner.run_round();
        let finished = outcome.winner_team.is_some();
        let Some(frame) = outcome.frame else {
            idle_rounds += 1;
            if finished || idle_rounds > 16 {
                break;
            }
            continue;
        };
        if frame.updates.updates.is_empty() {
            idle_rounds += 1;
            if finished || idle_rounds > 16 {
                break;
            }
            continue;
        }
        idle_rounds = 0;

        lines.push(format!("=== 回合 {round} ==="));
        for update in frame.updates.updates {
            match update.update_type {
                UpdateType::NextLine => lines.push(String::new()),
                _ => {
                    if update.score > 0 {
                        let score = u64::from(update.score);
                        total_score += score;
                        *score_by_caster.entry(update.caster).or_insert(0) += score;
                    }
                    lines.push(fmt_runtime_v2_update(runner, &update));
                }
            }
        }
        round += 1;
        if finished {
            break;
        }
    }

    lines.push(String::new());
    lines.push("=== 对局结果 ===".to_owned());
    if let Some(winner_team) = runner.runtime().world.winner_team() {
        lines.push("赢家:".to_owned());
        if let Some(winners) = runner.runtime().world.team_roster(winner_team) {
            for winner in winners {
                if let Some(entity) = runner.runtime().entities.get(*winner) {
                    let battle_score = score_by_caster.get(&(winner.0 as usize)).copied().unwrap_or(0);
                    let all_sum = entity.template.clone_build.as_ref().map_or(
                        entity
                            .template
                            .attr_sum
                            .saturating_mul(3)
                            .saturating_add(entity.template.max_hp.max(0) as u32),
                        |build| build.all_sum(),
                    );
                    lines.push(format!(
                        "- {} (id={}, all_sum={}, battle_score={}, hp={})",
                        entity.template.display_name, winner.0, all_sum, battle_score, entity.runtime.hp
                    ));
                }
            }
        }
    } else {
        lines.push("未分出胜负（达到安全轮次或连续空更新）。".to_owned());
    }
    lines.push(format!("总战斗分: {total_score}"));
    if let Some(win_idx_line) = fmt_runtime_v2_winner_input_indices(runner, input_player_count) {
        lines.push(win_idx_line);
    }
    lines
}

fn runtime_v2_normalized_json(raw: &str, max_rounds: usize) -> Result<String, String> {
    let run = core_cli_api::default_custom_runtime_v2_normalized_run(raw, max_rounds).map_err(cli_api_error)?;
    serde_json::to_string_pretty(&JsonRuntimeV2NormalizedRun::from(run))
        .map_err(|err| format!("序列化 runtime-v2 JSON 失败: {err}"))
}

fn runtime_v2_parity_json(raw: &str, max_rounds: usize) -> Result<String, String> {
    let report = core_cli_api::default_custom_runtime_v2_parity_report(raw, max_rounds).map_err(cli_api_error)?;
    serde_json::to_string_pretty(&JsonRuntimeV2ParityReport::from(report))
        .map_err(|err| format!("序列化 runtime-v2 parity JSON 失败: {err}"))
}

fn cli_api_error(err: CliApiError) -> String {
    match err {
        CliApiError::InvalidInput(message) => message,
        CliApiError::Runner(err) => format!("构建 v2 对局失败: {err}"),
        CliApiError::RuntimeV2(message) => format!("运行 v2 对局失败: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_v2_fight_lines_match_legacy_for_minimal_raw() {
        let raw = "left@red\n\nright@blue\n";
        let mut legacy = tswn_core::Runner::new_from_namerena_raw(raw.to_owned()).expect("legacy runner should build");
        let input_player_ids = super::super::driver::collect_input_player_ids(&legacy);
        let legacy_lines = super::super::driver::collect_legacy_fight_lines(&mut legacy, &input_player_ids, 100_000);

        let mut v2 = core_cli_api::default_custom_runtime_v2_mixed_runner(raw).expect("runtime v2 runner should build");
        let input_player_count = v2.runtime().entities.len();
        let v2_lines = collect_runtime_v2_fight_lines(&mut v2, input_player_count, 100_000);

        assert_eq!(v2_lines, legacy_lines);
    }

    #[test]
    fn runtime_v2_raw_lines_match_legacy_for_minimal_raw() {
        let raw = "left@red\n\nright@blue\n";
        let mut legacy = tswn_core::Runner::new_from_namerena_raw(raw.to_owned()).expect("legacy runner should build");
        let input_player_ids = super::super::driver::collect_input_player_ids(&legacy);
        let legacy_lines = super::super::trace::collect_fight_raw_lines(&mut legacy, &input_player_ids);

        let mut v2 = core_cli_api::default_custom_runtime_v2_mixed_runner(raw).expect("runtime v2 runner should build");
        let input_player_count = v2.runtime().entities.len();
        let v2_lines = collect_runtime_v2_fight_raw_lines(&mut v2, input_player_count);

        assert_eq!(v2_lines, legacy_lines);
    }

    #[test]
    fn runtime_v2_fight_outputs_match_legacy_for_minion_and_clan_raw() {
        let raw = "我力 7#W2ib8D@仙蛊屋+123\n万我 68#huMG43@仙蛊屋+123\n\n\
                   Dianmu YKFMWRPXIMCQ@nan+234\nFreddy FVNXBNVTWJEA@nan+234\n\n\
                   seed:第十八届武术大赛小组赛第8组:307-3@!\n";

        let mut legacy = tswn_core::Runner::new_from_namerena_raw(raw.to_owned()).expect("legacy runner should build");
        let input_player_ids = super::super::driver::collect_input_player_ids(&legacy);
        let legacy_fight = super::super::driver::collect_legacy_fight_lines(&mut legacy, &input_player_ids, 100_000);
        let mut v2 = core_cli_api::default_custom_runtime_v2_mixed_runner(raw).expect("runtime v2 runner should build");
        let input_player_count = v2.runtime().entities.len();
        let v2_fight = collect_runtime_v2_fight_lines(&mut v2, input_player_count, 100_000);
        assert_eq!(v2_fight, legacy_fight);

        let mut legacy = tswn_core::Runner::new_from_namerena_raw(raw.to_owned()).expect("legacy runner should build");
        let input_player_ids = super::super::driver::collect_input_player_ids(&legacy);
        let legacy_raw = super::super::trace::collect_fight_raw_lines(&mut legacy, &input_player_ids);
        let mut v2 = core_cli_api::default_custom_runtime_v2_mixed_runner(raw).expect("runtime v2 runner should build");
        let input_player_count = v2.runtime().entities.len();
        let v2_raw = collect_runtime_v2_fight_raw_lines(&mut v2, input_player_count);
        assert_eq!(v2_raw, legacy_raw);
    }

    #[test]
    fn runtime_v2_diff_lines_match_legacy_diff_for_minimal_raw() {
        let raw = "left@red\n\nright@blue\n";
        let lines = runtime_v2_diff_lines(raw, 1).expect("runtime v2 diff should render");

        let mut legacy = tswn_core::Runner::new_from_namerena_raw(raw.to_owned()).expect("legacy runner should build");
        let (legacy_lines, _guard, _total_score) = super::super::trace::collect_diff_lines(&mut legacy, 1, true);

        assert_eq!(lines, legacy_lines);
    }

    #[test]
    fn runtime_v2_normalized_json_matches_default_run_golden_shape() {
        let json = runtime_v2_normalized_json("left@red\n\nright@blue\n", 1).expect("runtime v2 json should serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("runtime v2 json should parse");

        assert_eq!(value["winner_team"], serde_json::Value::Null);
        assert_eq!(value["guard_exhausted"], true);
        assert_eq!(value["total_score"], 77);
        let rounds = value["rounds"].as_array().expect("rounds should be an array");
        assert_eq!(rounds.len(), 1);
        let round = &rounds[0];
        assert_eq!(round["winner_team"], serde_json::Value::Null);
        assert_eq!(round["round"], 1);
        assert_eq!(round["total_score"], 77);
        assert_eq!(round["rng_i"], 74);
        assert_eq!(round["rng_j"], 92);
        assert_eq!(round["entity_ids"], serde_json::json!([1, 2]));
        assert_eq!(round["teams"], serde_json::json!([0, 1]));
        assert_eq!(round["hp"], serde_json::json!([262, 288]));
        assert_eq!(round["magic_point"], serde_json::json!([23, 16]));
        assert_eq!(round["defense"], serde_json::json!([6, 56]));
        assert_eq!(round["resistance"], serde_json::json!([52, 25]));
        assert_eq!(round["alive"], serde_json::json!([true, true]));
        assert_eq!(round["round_order"], serde_json::json!([0, 1]));
        assert_eq!(round["flat_alive"], serde_json::json!([0, 1]));
        assert_eq!(round["team_alive"], serde_json::json!([[0], [1]]));
        assert_eq!(round["alive_group_count"], 2);
        assert_eq!(
            round["actions"],
            serde_json::json!([{ "round": 1, "actor": 1, "target": 0, "amount": 36 }])
        );
        assert_eq!(
            round["frames"],
            serde_json::json!([
                {
                    "message": "[0]发起攻击",
                    "caster": 1,
                    "target": 0,
                    "targets": [],
                    "param": null,
                    "score": 0,
                    "delay0": 1000,
                    "delay1": 100,
                    "update_type": "none"
                },
                {
                    "message": "[1]受到[2]点伤害",
                    "caster": 1,
                    "target": 0,
                    "targets": [],
                    "param": null,
                    "score": 77,
                    "delay0": 1154,
                    "delay1": 100,
                    "update_type": "none"
                },
                {
                    "message": "\n",
                    "caster": 0,
                    "target": 0,
                    "targets": [],
                    "param": null,
                    "score": 0,
                    "delay0": 0,
                    "delay1": 0,
                    "update_type": "next_line"
                }
            ])
        );
    }

    #[test]
    fn runtime_v2_normalized_json_rejects_zero_max_rounds() {
        let err =
            runtime_v2_normalized_json("left@red\n\nright@blue\n", 0).expect_err("runtime v2 json should reject zero max rounds");

        assert_eq!(err, "runtime v2 max_rounds must be positive");
    }

    #[test]
    fn runtime_v2_parity_json_reports_matching_converged_prefix_and_both_runs() {
        let json = runtime_v2_parity_json("left@red\n\nright@blue\n", 1).expect("runtime v2 parity json should serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("runtime v2 parity json should parse");

        assert_eq!(value["matched"], true);
        assert!(value["first_diff"].is_null());
        assert_eq!(value["legacy"]["rounds"].as_array().unwrap().len(), 1);
        assert_eq!(value["v2"]["rounds"].as_array().unwrap().len(), 1);

        #[cfg(not(feature = "no_debug"))]
        assert_eq!(value["legacy"], value["v2"]);

        #[cfg(feature = "no_debug")]
        {
            assert_eq!(value["legacy"]["total_score"], value["v2"]["total_score"]);
            assert_eq!(value["legacy"]["rounds"][0]["frames"], value["v2"]["rounds"][0]["frames"]);
            assert_eq!(value["legacy"]["rounds"][0]["rng"], value["v2"]["rounds"][0]["rng"]);
        }
    }

    #[test]
    fn runtime_v2_parity_json_rejects_zero_max_rounds() {
        let err = runtime_v2_parity_json("left@red\n\nright@blue\n", 0)
            .expect_err("runtime v2 parity json should reject zero max rounds");

        assert_eq!(err, "runtime v2 max_rounds must be positive");
    }
}

//! CLI 侧的主 Runtime 正式入口。

use std::collections::HashMap;

use super::driver::fmt_runtime_winner_input_indices;
use super::trace::{collect_runtime_diff_lines, fmt_runtime_update};
use tswn_core::cli_api::{self as core_cli_api, CliApiError, JsonRuntimeNormalizedRun};
use tswn_core::runtime::update::UpdateType;
use tswn_core::runtime::{EntityIdx, RuntimeRunner};

pub(super) fn run_runtime_fight(raw: String) {
    let mut runner = match core_cli_api::default_custom_runtime_mixed_runner(&raw).map_err(cli_api_error) {
        Ok(runner) => runner,
        Err(err) => {
            eprintln!("构建对局失败: {err}");
            std::process::exit(1);
        }
    };
    let input_player_count = runner.runtime().entities.len();
    let lines = collect_runtime_fight_lines(&mut runner, input_player_count, 100_000);
    if !lines.is_empty() {
        println!("{}", lines.join("\n"));
    }
}

pub(super) fn run_runtime_diff(raw: String) {
    match runtime_diff_lines(&raw, 20_000) {
        Ok(lines) if !lines.is_empty() => println!("{}", lines.join("\n")),
        Ok(_) => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

pub fn run_runtime_normalized(raw: String, max_rounds: usize) {
    match runtime_normalized_json(&raw, max_rounds) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn runtime_diff_lines(raw: &str, max_rounds: usize) -> Result<Vec<String>, String> {
    let mut runner = core_cli_api::default_custom_runtime_mixed_runner(raw).map_err(cli_api_error)?;
    let (lines, _, _) = collect_runtime_diff_lines(&mut runner, max_rounds, true);
    Ok(lines)
}

fn runtime_display_stats(runner: &RuntimeRunner, entity_idx: EntityIdx) -> Option<String> {
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

fn collect_runtime_fight_lines(runner: &mut RuntimeRunner, input_player_count: usize, max_rounds: usize) -> Vec<String> {
    let mut lines = vec!["=== 玩家状态 ===".to_owned()];
    for entity_idx in 0..input_player_count {
        let entity_idx = EntityIdx(entity_idx.try_into().expect("runtime CLI input entity index overflow"));
        if let Some(line) = runtime_display_stats(runner, entity_idx) {
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
            if matches!(update.update_type, UpdateType::NextLine) {
                lines.push(String::new());
            } else {
                if update.score > 0 {
                    let score = u64::from(update.score);
                    total_score += score;
                    *score_by_caster.entry(update.caster).or_insert(0) += score;
                }
                lines.push(fmt_runtime_update(runner, &update));
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
    if let Some(win_idx) = fmt_runtime_winner_input_indices(runner, input_player_count) {
        lines.push(win_idx);
    }
    lines
}

fn runtime_normalized_json(raw: &str, max_rounds: usize) -> Result<String, String> {
    let run = core_cli_api::default_custom_runtime_normalized_run(raw, max_rounds).map_err(cli_api_error)?;
    serde_json::to_string_pretty(&JsonRuntimeNormalizedRun::from(run)).map_err(|err| format!("序列化 runtime JSON 失败: {err}"))
}

fn cli_api_error(err: CliApiError) -> String {
    match err {
        CliApiError::InvalidInput(message)
        | CliApiError::InvalidArgument(message)
        | CliApiError::UnsupportedOption(message)
        | CliApiError::Internal(message) => message,
        CliApiError::RunnerInit(err) => format!("构建 runtime 对局失败: {err}"),
        CliApiError::Runtime(message) => format!("运行 runtime 对局失败: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str = "left@red\n\nright@blue\nseed:42@!";

    #[test]
    fn normalized_run_is_valid_json() {
        let json = runtime_normalized_json(RAW, 8).expect("normalized run should serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("normalized run should be JSON");
        assert!(
            value
                .get("rounds")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|rounds| !rounds.is_empty())
        );
    }

    #[test]
    fn fight_and_diff_outputs_are_deterministic() {
        let build = || core_cli_api::default_custom_runtime_mixed_runner(RAW).expect("runner should build");
        let mut first = build();
        let count = first.runtime().entities.len();
        let first_fight = collect_runtime_fight_lines(&mut first, count, 20_000);
        let mut second = build();
        let second_fight = collect_runtime_fight_lines(&mut second, count, 20_000);
        assert_eq!(first_fight, second_fight);

        assert_eq!(runtime_diff_lines(RAW, 20_000), runtime_diff_lines(RAW, 20_000));
    }
}

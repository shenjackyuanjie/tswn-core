//! CLI 侧的 runtime v2 迁移/调试入口。

use serde::Serialize;
use tswn_core::cli_api::{self as core_cli_api, CliApiError};
use tswn_core::engine::update::UpdateType;
use tswn_core::runtime_v2::{NormalizedOutcome, NormalizedUpdateFrame, RuntimeV2NormalizedRun};

#[derive(Serialize)]
struct JsonRuntimeV2NormalizedRun {
    rounds: Vec<JsonRuntimeV2NormalizedOutcome>,
    winner_team: Option<usize>,
    guard_exhausted: bool,
    total_score: u64,
}

#[derive(Serialize)]
struct JsonRuntimeV2NormalizedOutcome {
    winner_team: Option<usize>,
    round: u64,
    total_score: u64,
    rng_i: u32,
    rng_j: u32,
    entity_ids: Vec<usize>,
    teams: Vec<usize>,
    hp: Vec<i32>,
    magic_point: Vec<i32>,
    defense: Vec<i32>,
    resistance: Vec<i32>,
    alive: Vec<bool>,
    round_order: Vec<usize>,
    flat_alive: Vec<usize>,
    team_alive: Vec<Vec<usize>>,
    alive_group_count: usize,
    actions: Vec<JsonRuntimeV2ActionBoundary>,
    frames: Vec<JsonRuntimeV2UpdateFrame>,
}

#[derive(Serialize)]
struct JsonRuntimeV2ActionBoundary {
    round: u64,
    actor: usize,
    target: usize,
    amount: i32,
}

#[derive(Serialize)]
struct JsonRuntimeV2UpdateFrame {
    message: String,
    caster: usize,
    target: usize,
    targets: Vec<usize>,
    param: Option<u32>,
    score: u32,
    delay0: i32,
    delay1: i32,
    update_type: &'static str,
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

fn runtime_v2_normalized_json(raw: &str, max_rounds: usize) -> Result<String, String> {
    let run = core_cli_api::default_custom_runtime_v2_normalized_run(raw, max_rounds).map_err(cli_api_error)?;
    serde_json::to_string_pretty(&JsonRuntimeV2NormalizedRun::from(run))
        .map_err(|err| format!("序列化 runtime-v2 JSON 失败: {err}"))
}

fn cli_api_error(err: CliApiError) -> String {
    match err {
        CliApiError::InvalidInput(message) => message,
        CliApiError::Runner(err) => format!("构建 v2 对局失败: {err}"),
    }
}

impl From<RuntimeV2NormalizedRun> for JsonRuntimeV2NormalizedRun {
    fn from(value: RuntimeV2NormalizedRun) -> Self {
        Self {
            rounds: value.rounds.into_iter().map(Into::into).collect(),
            winner_team: value.winner_team,
            guard_exhausted: value.guard_exhausted,
            total_score: value.total_score,
        }
    }
}

impl From<NormalizedOutcome> for JsonRuntimeV2NormalizedOutcome {
    fn from(value: NormalizedOutcome) -> Self {
        Self {
            winner_team: value.winner_team,
            round: value.round,
            total_score: value.total_score,
            rng_i: value.rng.i,
            rng_j: value.rng.j,
            entity_ids: value.entity_ids,
            teams: value.teams,
            hp: value.hp,
            magic_point: value.magic_point,
            defense: value.defense,
            resistance: value.resistance,
            alive: value.alive,
            round_order: value.round_order,
            flat_alive: value.flat_alive,
            team_alive: value.team_alive,
            alive_group_count: value.alive_group_count,
            actions: value
                .actions
                .into_iter()
                .map(|action| JsonRuntimeV2ActionBoundary {
                    round: action.round,
                    actor: action.actor,
                    target: action.target,
                    amount: action.amount,
                })
                .collect(),
            frames: value.frames.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<NormalizedUpdateFrame> for JsonRuntimeV2UpdateFrame {
    fn from(value: NormalizedUpdateFrame) -> Self {
        Self {
            message: value.message,
            caster: value.caster,
            target: value.target,
            targets: value.targets,
            param: value.param,
            score: value.score,
            delay0: value.delay0,
            delay1: value.delay1,
            update_type: update_type_name(value.update_type),
        }
    }
}

fn update_type_name(value: UpdateType) -> &'static str {
    match value {
        UpdateType::Win => "win",
        UpdateType::None => "none",
        UpdateType::NextLine => "next_line",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_v2_normalized_json_contains_default_run_fields() {
        let json = runtime_v2_normalized_json("left@red\n\nright@blue\n", 1).expect("runtime v2 json should serialize");

        assert!(json.contains("\"rounds\""));
        assert!(json.contains("\"total_score\""));
        assert!(json.contains("\"update_type\""));
    }

    #[test]
    fn runtime_v2_update_type_names_are_stable_json_tokens() {
        assert_eq!(update_type_name(UpdateType::Win), "win");
        assert_eq!(update_type_name(UpdateType::None), "none");
        assert_eq!(update_type_name(UpdateType::NextLine), "next_line");
    }
}

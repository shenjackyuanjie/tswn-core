//! CLI 侧的 runtime v2 迁移/调试入口。

use serde::Serialize;
use tswn_core::cli_api::{self as core_cli_api, CliApiError, RuntimeV2ParityReport};
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
struct JsonRuntimeV2ParityReport {
    matched: bool,
    first_diff: Option<String>,
    legacy: JsonRuntimeV2NormalizedRun,
    v2: JsonRuntimeV2NormalizedRun,
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

pub fn run_runtime_v2_parity(raw: String, max_rounds: usize) {
    match runtime_v2_parity_json(&raw, max_rounds) {
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

fn runtime_v2_parity_json(raw: &str, max_rounds: usize) -> Result<String, String> {
    let report = core_cli_api::default_custom_runtime_v2_parity_report(raw, max_rounds).map_err(cli_api_error)?;
    serde_json::to_string_pretty(&JsonRuntimeV2ParityReport::from(report))
        .map_err(|err| format!("序列化 runtime-v2 parity JSON 失败: {err}"))
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

impl From<RuntimeV2ParityReport> for JsonRuntimeV2ParityReport {
    fn from(value: RuntimeV2ParityReport) -> Self {
        let RuntimeV2ParityReport { legacy, v2, first_diff } = value;
        Self {
            matched: first_diff.is_none(),
            first_diff: first_diff.map(|diff| format!("{diff:?}")),
            legacy: legacy.into(),
            v2: v2.into(),
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

    #[test]
    fn runtime_v2_update_type_names_are_stable_json_tokens() {
        assert_eq!(update_type_name(UpdateType::Win), "win");
        assert_eq!(update_type_name(UpdateType::None), "none");
        assert_eq!(update_type_name(UpdateType::NextLine), "next_line");
    }
}

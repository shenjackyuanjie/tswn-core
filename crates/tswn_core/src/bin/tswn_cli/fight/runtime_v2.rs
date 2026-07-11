//! CLI 侧的 runtime v2 迁移/调试入口。

use tswn_core::cli_api::{self as core_cli_api, CliApiError, JsonRuntimeV2NormalizedRun, JsonRuntimeV2ParityReport};

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
}

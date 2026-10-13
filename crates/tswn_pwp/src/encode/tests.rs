//! 验证实际编码文件、监督标签和源行关联，不检查源码写法。
use super::*;
use crate::{BattleRow, input, manifest, random};
use clap::Parser;
use serde_json::json;
use tempfile::TempDir;
use tswn_core::{
    cli_api::battle::{BattleModelOutcome, BattleStopReason},
    encoder::encode::required_normalization,
    runtime::RuntimeRunner,
};

// 这里只验证导出协议，使用完整的合成统计；不能作为训练归一化常数发布。
fn calibration() -> serde_json::Value {
    let fields: serde_json::Map<String, serde_json::Value> = required_normalization()
        .into_iter()
        .map(|(path, _)| {
            (
                path.to_owned(),
                json!({
                    "count": 4, "min": 0.0, "max": 1.0, "p50": 1.0, "p99": 1.0,
                    "abs_p50": 1.0, "abs_p99": 1.0, "s_f": 1.0, "c_f": 1.0,
                }),
            )
        })
        .collect();
    json!({
        "schema": "tswn-pwp/encoder-calibration", "schema_version": 1, "state_schema_version": 1,
        "format_version": 1, "split": "train", "require_label": true,
        "samples_seen": 4, "samples_selected": 3, "input_sha256": "test-input",
        "executable_sha256": "test-executable", "selected_rows_digest": "test-rows",
        "calibrator": "合成导出测试", "fields": fields,
    })
}

fn fixture(root: &Path) -> (EncodeArgs, Vec<SampleRow>) {
    let calibration_path = root.join("calibration.json");
    storage::write_json(&calibration_path, &calibration()).unwrap();
    let manifest_path = root.join("encoder.json");
    manifest::run(&manifest::BuildManifestArgs {
        calibration: calibration_path,
        out: manifest_path.clone(),
    })
    .unwrap();
    let dataset = root.join("dataset");
    fs::create_dir(&dataset).unwrap();
    let groups = (0..100)
        .map(|i| vec![vec![format!("left-{i}")], vec!["right".to_owned()]])
        .find(|groups| random::split(&input::matchup_id(groups)) == "train")
        .unwrap();
    let matchup_id = input::matchup_id(&groups);
    let config = DatasetConfig {
        format_version: 1,
        state_schema_version: 1,
        seed: "training-export".into(),
        games_per_matchup: 4,
        samples_per_game: 1,
        battles_per_shard: 2,
        max_rounds: 100,
        eval_rq: tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ,
        input_sha256: "test-input".into(),
        input_mode: "fixed".into(),
        team_sizes: vec![],
        matchups: None,
        executable_sha256: "test-executable".into(),
        cases: vec![input::Case {
            source: "合成导出测试".into(),
            original_input: groups.iter().map(|g| g.join("\n")).collect::<Vec<_>>().join("\n\n"),
            groups,
            matchup_id: matchup_id.clone(),
        }],
    };
    storage::write_json(&dataset.join("manifest.json"), &config).unwrap();
    let runner = RuntimeRunner::new_from_namerena_raw(config.cases[0].original_input.clone()).unwrap();
    let initial = runner.model_state().unwrap();
    let rows: Vec<_> = [Some(0), None, Some(1), Some(0)]
        .into_iter()
        .enumerate()
        .map(|(i, winner)| {
            let mut state = initial.clone();
            state.entities[0].runtime.hp -= i as i32;
            SampleRow {
                battle_id: i as u64,
                matchup_id: matchup_id.clone(),
                split: "train".into(),
                frame: None,
                rounds_advanced: 0,
                progress: 0.0,
                winner_team_index: winner,
                state,
            }
        })
        .collect();
    for (index, chunk) in rows.chunks(2).enumerate() {
        let dir = dataset.join(format!("shard-{index:06}"));
        fs::create_dir(&dir).unwrap();
        let mut writer = storage::TableWriter::<SampleRow>::create(&dir.join("samples.parquet")).unwrap();
        writer.append(chunk).unwrap();
        writer.finish().unwrap();
        let battles: Vec<_> = chunk
            .iter()
            .map(|row| BattleRow {
                battle_id: row.battle_id,
                case_index: 0,
                matchup_id: matchup_id.clone(),
                split: "train".into(),
                seed: random::battle_seed(&config.seed, &matchup_id, row.battle_id),
                outcome: BattleModelOutcome {
                    stop_reason: if row.winner_team_index.is_some() {
                        BattleStopReason::Winner
                    } else {
                        BattleStopReason::MaxRounds
                    },
                    rounds_advanced: if row.winner_team_index.is_some() { 1 } else { 100 },
                    frames_emitted: 1,
                    winner_team_indices: row.winner_team_index.into_iter().collect(),
                },
                samples: 1,
            })
            .collect();
        let mut writer = storage::TableWriter::<BattleRow>::create(&dir.join("battles.parquet")).unwrap();
        writer.append(&battles).unwrap();
        writer.finish().unwrap();
        write_receipt(&dir, index);
    }
    (
        EncodeArgs {
            dataset,
            manifest: manifest_path,
            out: root.join("encoded"),
            split: "train".into(),
            batch_size: 2,
            keep_unlabeled: false,
        },
        rows,
    )
}

fn write_receipt(dir: &Path, index: usize) {
    storage::write_json(
        &dir.join("complete.json"),
        &storage::ShardReceipt {
            first_battle: index * 2,
            end_battle: index * 2 + 2,
            battles: 2,
            samples: 2,
            samples_sha256: storage::file_hash(&dir.join("samples.parquet")).unwrap(),
            battles_sha256: storage::file_hash(&dir.join("battles.parquet")).unwrap(),
        },
    )
    .unwrap();
}

#[test]
fn build_manifest_cli_publishes_usable_contract_and_preserves_output_on_bad_report() {
    let root = TempDir::new().unwrap();
    let report = root.path().join("report.json");
    let out = root.path().join("contract.json");
    storage::write_json(&report, &calibration()).unwrap();
    let cli = crate::Cli::try_parse_from([
        "tswn-pwp",
        "build-manifest",
        "--calibration",
        report.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ])
    .unwrap();
    crate::run(cli).unwrap();
    let manifest: EncoderManifest = storage::read_json(&out).unwrap();
    assert_eq!(manifest.contract_digest, Some(manifest.digest()));
    FeatureEncoder::new(manifest).unwrap();
    let before = fs::read(&out).unwrap();
    let args = manifest::BuildManifestArgs {
        calibration: report.clone(),
        out: out.clone(),
    };
    let mut incomplete = calibration();
    incomplete["fields"].as_object_mut().unwrap().remove("global.round");
    storage::write_json(&report, &incomplete).unwrap();
    assert!(manifest::run(&args).unwrap_err().to_string().contains("global.round"));
    assert_eq!(fs::read(&out).unwrap(), before);
    let mut wrong_split = calibration();
    wrong_split["split"] = json!("validation");
    storage::write_json(&report, &wrong_split).unwrap();
    assert!(manifest::run(&args).is_err());
    assert_eq!(fs::read(&out).unwrap(), before);
}

#[test]
fn export_aligns_labels_source_rows_tail_and_core_bytes() {
    let root = TempDir::new().unwrap();
    let (args, rows) = fixture(root.path());
    run(&args).unwrap();
    let export: ExportManifest = storage::read_json(&args.out.join("manifest.json")).unwrap();
    assert_eq!((export.samples, export.batches), (3, 2));
    assert!(!export.keep_unlabeled);
    let manifest: EncoderManifest = storage::read_json(&args.out.join("encoder-manifest.json")).unwrap();
    assert_eq!(manifest, storage::read_json::<EncoderManifest>(&args.manifest).unwrap());
    let encoder = FeatureEncoder::new(manifest).unwrap();
    let mut output_rows = Vec::new();
    for index in 0..export.batches {
        let dir = args.out.join(format!("batch-{index:06}"));
        let batch: BatchManifest = storage::read_json(&dir.join("batch-manifest.json")).unwrap();
        let audit: Vec<SourceRow> = storage::read_json(&dir.join(&batch.audit.file)).unwrap();
        let winners = fs::read(dir.join("winner_team_index.bin")).unwrap();
        let masks = fs::read(dir.join("label_mask.bin")).unwrap();
        assert_eq!(audit.len(), batch.batch_size);
        assert_eq!(masks, vec![1; batch.batch_size]);
        for (slot, source) in audit.iter().enumerate() {
            let original = &rows[source.source_shard * 2 + source.source_row];
            assert_eq!(source.battle_id, original.battle_id);
            assert_eq!(
                i32::from_le_bytes(winners[slot * 4..slot * 4 + 4].try_into().unwrap()),
                original.winner_team_index.unwrap() as i32
            );
            let single = encoder.encode(&original.state).unwrap();
            for tensor in batch.features.iter().chain(&batch.exact) {
                let bytes = fs::read(dir.join(&tensor.payload.file)).unwrap();
                let expected = single.tensor_bytes(&tensor.name).unwrap();
                assert_eq!(
                    &bytes[slot * expected.len()..(slot + 1) * expected.len()],
                    expected,
                    "{}",
                    tensor.name
                );
                assert_eq!(tensor.shape[0], batch.batch_size);
            }
        }
        output_rows.extend(audit.into_iter().map(|source| (source.source_shard, source.source_row)));
    }
    assert_eq!(output_rows, vec![(0, 0), (1, 0), (1, 1)]);
    // 有 Python 时检验实际消费端；纯 Rust 环境不额外要求解释器。
    if std::process::Command::new("python")
        .arg("--version")
        .output()
        .is_ok_and(|result| result.status.success())
    {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/read_encoded_batches.py");
        let result = std::process::Command::new("python").arg(script).arg(&args.out).output().unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    }
}

#[test]
fn diagnostic_export_retains_null_label_with_zero_mask() {
    let root = TempDir::new().unwrap();
    let (mut args, _) = fixture(root.path());
    args.keep_unlabeled = true;
    run(&args).unwrap();
    let export: ExportManifest = storage::read_json(&args.out.join("manifest.json")).unwrap();
    assert_eq!((export.samples, export.batches), (4, 2));
    let dir = args.out.join("batch-000000");
    assert_eq!(fs::read(dir.join("label_mask.bin")).unwrap(), vec![1, 0]);
    let bytes = fs::read(dir.join("winner_team_index.bin")).unwrap();
    assert_eq!(i32::from_le_bytes(bytes[4..8].try_into().unwrap()), -1);
    let audit: Vec<SourceRow> = storage::read_json(&dir.join("rows.json")).unwrap();
    assert_eq!(audit[1].source_row, 1);
}

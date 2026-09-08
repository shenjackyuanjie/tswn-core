use super::*;
use std::fs;
use tempfile::TempDir;

fn args(root: &std::path::Path) -> GenerateArgs {
    let input = root.join("case.txt");
    fs::write(&input, "left@red\nfriend@red\n\nright@blue\n\nthird@green\nseed:ignored").unwrap();
    GenerateArgs {
        input: Some(input),
        names: None,
        team_sizes: Vec::new(),
        matchups: None,
        games_per_matchup: 4,
        seed: "dataset-tests".into(),
        out: root.join("out"),
        samples_per_game: 8,
        battles_per_shard: 2,
        threads: 1,
        max_rounds: 100,
        eval_rq: tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ,
        resume: false,
    }
}

fn sample_rows(out: &std::path::Path) -> Vec<SampleRow> {
    let mut rows = Vec::new();
    for shard in [0, 1] {
        storage::read_rows(&out.join(format!("shard-{shard:06}/samples.parquet")), |row| {
            rows.push(row);
            Ok(())
        })
        .unwrap();
    }
    rows
}

#[test]
fn parquet_roundtrip_threads_resume_and_corruption() {
    let root = TempDir::new().unwrap();
    let mut options = args(root.path());
    generate::generate(&options).unwrap();
    let original = sample_rows(&options.out);
    assert!(!original.is_empty());
    assert!(original.iter().all(|row| row.state.validate().is_ok()));
    let summary = validate::validate_dataset(&options.out).unwrap();
    assert_eq!(summary.battles, 4);
    // Rust 自己回读不能覆盖跨实现的嵌套字典问题；装有 PyArrow 时额外验证实际 Python 消费方。
    if std::process::Command::new("python")
        .args(["-c", "import pyarrow"])
        .output()
        .is_ok_and(|output| output.status.success())
    {
        let output = std::process::Command::new("python")
            .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/read_winprob_dataset.py"))
            .arg(&options.out)
            .arg("--include-unresolved")
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stdout).contains(&original.len().to_string()));
    }

    let first_dir = options.out.clone();
    options.out = root.path().join("parallel");
    options.threads = 2;
    generate::generate(&options).unwrap();
    assert_eq!(sample_rows(&options.out), original);
    for index in [0, 1] {
        assert_eq!(
            storage::file_hash(&first_dir.join(format!("shard-{index:06}/samples.parquet"))).unwrap(),
            storage::file_hash(&options.out.join(format!("shard-{index:06}/samples.parquet"))).unwrap()
        );
    }

    // 模拟最后一个分片尚未提交便退出：已完成分片必须跳过，临时分片重做。
    let completed = options.out.join("shard-000001");
    let temporary = options.out.join(".shard-000001.tmp");
    fs::rename(completed, &temporary).unwrap();
    fs::write(temporary.join("samples.parquet"), b"interrupted").unwrap();
    let first_hash = storage::file_hash(&options.out.join("shard-000000/samples.parquet")).unwrap();
    options.resume = true;
    generate::generate(&options).unwrap();
    assert_eq!(sample_rows(&options.out), original);
    assert_eq!(
        storage::file_hash(&options.out.join("shard-000000/samples.parquet")).unwrap(),
        first_hash
    );
    options.seed = "different".into();
    assert!(generate::generate(&options).unwrap_err().to_string().contains("不能混合续跑"));
    options.seed = "dataset-tests".into();
    fs::write(options.out.join("shard-000000/samples.parquet"), b"corrupted").unwrap();
    assert!(validate::validate_dataset(&options.out).is_err());
    assert!(generate::generate(&options).is_err());
}

#[test]
fn names_mode_and_truncation_keep_null_labels() {
    let root = TempDir::new().unwrap();
    let mut options = args(root.path());
    let names = root.path().join("names.txt");
    fs::write(&names, "alpha\nbeta\ngamma\ndelta\nalpha\n").unwrap();
    options.input = None;
    options.names = Some(names);
    options.team_sizes = vec![1, 2];
    options.matchups = Some(2);
    options.games_per_matchup = 2;
    options.max_rounds = 1;
    generate::generate(&options).unwrap();
    let rows = sample_rows(&options.out);
    assert!(rows.iter().all(|row| row.winner_team_index.is_none()));
    let config: DatasetConfig = storage::read_json(&options.out.join("manifest.json")).unwrap();
    for case in config.cases {
        let names: std::collections::BTreeSet<_> = case.groups.iter().flatten().collect();
        assert_eq!(names.len(), 3);
    }
    assert_eq!(validate::validate_dataset(&options.out).unwrap().truncated, 4);
}

#[test]
fn runtime_panic_preserves_reproduction_and_never_commits_shard() {
    let root = TempDir::new().unwrap();
    let mut options = args(root.path());
    // 当前低层自定义 bed2 允许缺少召唤模板；执行召唤时会触发引擎错误。
    fs::write(options.input.as_ref().unwrap(), "alpha@red+bed2[3000]\n\nbeta@blue").unwrap();
    options.games_per_matchup = 1;
    options.max_rounds = 500;
    assert!(generate::generate(&options).is_err());
    assert!(!options.out.join("shard-000000").exists());
    assert!(options.out.join(".shard-000000.tmp/active-battle.json").exists());
    assert!(options.out.join("failure.json").exists());
}

#[test]
fn wide_state_rows_share_row_groups_instead_of_one_per_append() {
    // 超宽嵌套 state 的字节估算会被列缓冲容量放大；回归保护：行组按行数切分。
    use tswn_core::runtime::RuntimeRunner;
    let runner = RuntimeRunner::new_from_namerena_raw("left\n\nright".into()).unwrap();
    let row = SampleRow {
        battle_id: 0,
        matchup_id: "row-group-probe".into(),
        split: "train".into(),
        frame: None,
        rounds_advanced: 0,
        progress: 0.0,
        winner_team_index: Some(0),
        state: runner.model_state().unwrap(),
    };
    let root = TempDir::new().unwrap();
    let file = root.path().join("samples.parquet");
    let mut writer = storage::TableWriter::<SampleRow>::create(&file).unwrap();
    // 生成器每局调用一次 append；修复前列数估算会让每次 append 都新开一个行组。
    for _ in 0..16 {
        writer.append(std::slice::from_ref(&row)).unwrap();
    }
    writer.finish().unwrap();
    let builder = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(fs::File::open(&file).unwrap()).unwrap();
    assert_eq!(builder.metadata().file_metadata().num_rows(), 16);
    assert_eq!(
        builder.metadata().num_row_groups(),
        1,
        "每局一个行组会把压缩比与回读开销放大两个数量级"
    );
    assert!(storage::ROW_GROUP_ROWS > 16);
}

#[test]
fn cli_rejects_ambiguous_input_modes() {
    use clap::Parser;
    assert!(
        Cli::try_parse_from([
            "test",
            "generate",
            "--input",
            "a",
            "--names",
            "b",
            "--games-per-matchup",
            "1",
            "--seed",
            "s",
            "--out",
            "out"
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "test",
            "generate",
            "--names",
            "b",
            "--games-per-matchup",
            "1",
            "--seed",
            "s",
            "--out",
            "out"
        ])
        .is_err()
    );
}

#[test]
fn parquet_preserves_rare_payloads_and_full_integer_precision() {
    use tswn_core::runtime::{CovidInfectionEntry, EntityIdx, RuntimeRunner, StateEntry, StatePayload};
    let mut runner = RuntimeRunner::new_from_namerena_raw("left\n\nright".into()).unwrap();
    let payloads = [
        StatePayload::None,
        StatePayload::FireMagHalfSteps(3),
        StatePayload::Ice { frozen_step: 2 },
        StatePayload::ShieldValue(700),
        StatePayload::Curse { prob: 7, multiply: 11 },
        StatePayload::Poison {
            caster: Some(1),
            target: Some(0),
            atp_bits: u64::MAX - 1,
            count: 4,
        },
        StatePayload::Haste {
            faster: 8,
            effective_faster: 9,
            step: 10,
        },
        StatePayload::Berserk { step: 5 },
        StatePayload::Charm {
            group_id: 1,
            effective_team_idx: Some(0),
            source_team_idx: Some(1),
            target: Some(1),
            step: 4,
        },
        StatePayload::Slow { step: 6 },
        StatePayload::Iron { protect: 5, step: 2 },
        StatePayload::CovidBoss { mutation: 9 },
        StatePayload::CovidInfection {
            entries: vec![CovidInfectionEntry {
                boss: EntityIdx(1),
                mutation: 9,
                days: 5,
            }]
            .into(),
            mutation_set: vec![9, 11].into(),
            recovered: true,
        },
        StatePayload::SaitamaBoss {
            turns: 80,
            damages: 5,
            hitters: vec![EntityIdx(1)].into(),
            minions: vec![EntityIdx(0)].into(),
        },
        StatePayload::LazyBoss { at_boost_bits: u64::MAX },
        StatePayload::LazyInfection { boss: EntityIdx(1) },
    ];
    for (index, payload) in payloads.into_iter().enumerate() {
        let mut entry = StateEntry::legacy(1000 + index as u32);
        entry.payload = payload;
        runner.runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(entry);
    }
    let row = SampleRow {
        battle_id: 0,
        matchup_id: "codec-test".into(),
        split: "train".into(),
        frame: None,
        rounds_advanced: 0,
        progress: 0.0,
        winner_team_index: Some(0),
        state: runner.model_state().unwrap(),
    };
    let root = TempDir::new().unwrap();
    let file = root.path().join("payloads.parquet");
    let mut writer = storage::TableWriter::<SampleRow>::create(&file).unwrap();
    writer.append(std::slice::from_ref(&row)).unwrap();
    writer.finish().unwrap();
    let mut rows = Vec::new();
    storage::read_rows::<SampleRow>(&file, |row| {
        rows.push(row);
        Ok(())
    })
    .unwrap();
    assert_eq!(rows, vec![row]);
}

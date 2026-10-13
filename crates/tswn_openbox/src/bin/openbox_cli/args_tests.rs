use std::path::PathBuf;

use super::*;
use tswn_openbox::backend::NamerPfMetric;

fn plan_of(argv: &[&str]) -> Result<Job, clap::Error> {
    let cli = Cli::try_parse_from(std::iter::once("openbox-cli").chain(argv.iter().copied())).expect("参数应能解析");
    cli.plan()
}

#[test]
fn to_diy_plan_defaults_details_on() {
    let job = plan_of(&["to-diy", "-r", "mario@team"]).unwrap();
    match job {
        Job::ToDiy(plan) => {
            assert_eq!(plan.raw, "mario@team");
            assert!(plan.details);
            assert!(!plan.old);
            assert!(!plan.minions);
            assert!(plan.output_file.is_none());
        }
        _ => panic!("unexpected job"),
    }
}

#[test]
fn to_diy_plan_keeps_details_off_for_batch_files() {
    let path = temp_file("to_diy_details_off.txt", "mario@team\nluigi@team");
    let from_file = plan_of(&["to-diy", "-f", path.to_str().unwrap()]).unwrap();
    let no_details = plan_of(&["to-diy", "-r", "mario@team", "--no-details"]).unwrap();
    let _ = std::fs::remove_file(&path);

    match from_file {
        Job::ToDiy(plan) => assert!(!plan.details, "-f 批量模式不应输出详情"),
        _ => panic!("unexpected job"),
    }
    match no_details {
        Job::ToDiy(plan) => assert!(!plan.details, "--no-details 应关闭详情"),
        _ => panic!("unexpected job"),
    }
}

#[test]
fn to_diy_rejects_old_with_minions() {
    let err = Cli::try_parse_from(["openbox-cli", "to-diy", "-r", "mario", "--old", "--minions"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn namer_pf_defaults_to_all_five_screen_metrics() {
    let job = plan_of(&["namer-pf", "-r", "mario"]).unwrap();
    match job {
        Job::NamerPf(plan) => {
            let labels = plan.metrics.iter().map(|m| m.metric).collect::<Vec<_>>();
            assert_eq!(labels, NamerPfMetric::ALL.to_vec());
            assert!(plan.metrics.iter().all(|m| m.screen && m.output_file.is_none()));
            assert!(!plan.no_screen);
            assert!(!plan.skill_board.enabled);
            assert_eq!(plan.count, DEFAULT_COUNT);
        }
        _ => panic!("unexpected job"),
    }
}

#[test]
fn namer_pf_keep_rq_defaults_off() {
    let job = plan_of(&["namer-pf", "-r", "mario"]).unwrap();
    match job {
        Job::NamerPf(plan) => assert!(!plan.keep_rq),
        _ => panic!("unexpected job"),
    }
    let job = plan_of(&["namer-pf", "-r", "mario", "--keep-rq"]).unwrap();
    match job {
        Job::NamerPf(plan) => assert!(plan.keep_rq),
        _ => panic!("unexpected job"),
    }
    let job = plan_of(&["namer-pf", "-r", "mario", "--keep-rq=false"]).unwrap();
    match job {
        Job::NamerPf(plan) => assert!(!plan.keep_rq),
        _ => panic!("unexpected job"),
    }
}

#[test]
fn namer_pf_metrics_normalize_order_and_files() {
    let job = plan_of(&["namer-pf", "-r", "mario", "--metric", "sum:30000", "--metric", "pp:8000"]).unwrap();
    match job {
        Job::NamerPf(plan) => {
            let labels = plan.metrics.iter().map(|m| m.metric).collect::<Vec<_>>();
            assert_eq!(labels, vec![NamerPfMetric::Pp, NamerPfMetric::Sum]);
            assert_eq!(plan.metrics[0].min_screen, Some(8000.0));
        }
        _ => panic!("unexpected job"),
    }
}

#[test]
fn namer_pf_rejects_duplicate_metric_and_shared_file() {
    let err = plan_of(&["namer-pf", "-r", "mario", "--metric", "pp", "--metric", "pp"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    let err = plan_of(&[
        "namer-pf",
        "-r",
        "mario",
        "--metric",
        "pp::same.txt",
        "--metric",
        "sum::same.txt",
    ])
    .unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
}

#[test]
fn namer_pf_no_screen_requires_file_output() {
    let err = plan_of(&["namer-pf", "-r", "mario", "--no-screen"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    // 有文件落点时允许。
    let job = plan_of(&["namer-pf", "-r", "mario", "--no-screen", "--metric", "pp::pp.txt"]).unwrap();
    match job {
        Job::NamerPf(plan) => {
            assert!(plan.no_screen);
            assert!(plan.metrics.iter().all(|m| !m.screen));
        }
        _ => panic!("unexpected job"),
    }
}

#[test]
fn namer_pf_skill_board_flags() {
    // --skill-board-out 依赖 --skill-board。
    let err = plan_of(&["namer-pf", "-r", "mario", "--skill-board-out", "b.txt"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    // 不带值 = 默认路径；带值 = 显式路径。
    let job = plan_of(&["namer-pf", "-r", "mario", "--skill-board"]).unwrap();
    match job {
        Job::NamerPf(plan) => {
            assert!(plan.skill_board.enabled);
            assert!(plan.skill_board.config.is_none());
            assert!(plan.skill_board.output_file.is_none());
        }
        _ => panic!("unexpected job"),
    }
    let job = plan_of(&["namer-pf", "-r", "mario", "--skill-board", "board.toml"]).unwrap();
    match job {
        Job::NamerPf(plan) => assert_eq!(plan.skill_board.config, Some(PathBuf::from("board.toml"))),
        _ => panic!("unexpected job"),
    }
}

/// 写临时文件并返回路径；文件带自增序号，并行测试互不覆盖。
fn temp_file(name: &str, content: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("tswn_openbox_cli_{}_{}_{}", std::process::id(), name, seq));
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn cqd_plan_reads_manual_files_with_defaults() {
    let targets = temp_file("cqd_targets.txt", "1@a\n2@b");
    let players = temp_file("cqd_players.txt", "3@c");
    let job = plan_of(&["cqd", "-l", targets.to_str().unwrap(), "-p", players.to_str().unwrap()]).unwrap();
    match job {
        Job::Cqd(plan) => {
            assert_eq!(plan.target_text, "1@a\n2@b");
            assert_eq!(plan.player_text, "3@c");
            // GUI 默认勾选：每组胜率明细开、keep-rq 开、wr 精度 3。
            assert!(plan.show_matchups);
            assert!(plan.keep_rq);
            assert_eq!(plan.wr_precision, 3);
            assert!(!plan.target_factor_enabled);
            assert!(!plan.target_double_plus);
        }
        _ => panic!("unexpected job"),
    }
}

#[test]
fn cqd_plan_can_disable_matchups_and_pick_output_mode() {
    let targets = temp_file("cqd_targets.txt", "1@a");
    let players = temp_file("cqd_players.txt", "3@c");
    let job = plan_of(&[
        "cqd",
        "-l",
        targets.to_str().unwrap(),
        "-p",
        players.to_str().unwrap(),
        "--no-show-matchups",
        "--pure",
        "-o",
        "out.txt",
        "--min-file",
        "60.5",
        "--double-plus",
    ])
    .unwrap();
    match job {
        Job::Cqd(plan) => {
            assert!(!plan.show_matchups);
            assert_eq!(plan.output.mode, OutputMode::Pure);
            assert!(plan.target_double_plus);
            assert_eq!(plan.min_file, Some(60.5));
        }
        _ => panic!("unexpected job"),
    }
}

#[test]
fn pair_plan_manual_teammates_defaults() {
    let targets = temp_file("pair_targets.txt", "1@a");
    let players = temp_file("pair_players.txt", "3@c");
    let mates = temp_file("pair_mates.txt", "5@e");
    let job = plan_of(&[
        "pair",
        "-l",
        targets.to_str().unwrap(),
        "-p",
        players.to_str().unwrap(),
        "--teammates",
        mates.to_str().unwrap(),
    ])
    .unwrap();
    match job {
        Job::Pair(plan) => {
            // 手动队友：head 默认 3、队友默认 `++` 分隔（GUI 同）、detail 默认 every。
            assert_eq!(plan.head, 3);
            assert!(plan.teammate_double_plus);
            assert_eq!(plan.detail, PairDetailMode::Every);
            assert!(!plan.teammate_factor_enabled);
            assert!(!plan.player_double_plus);
        }
        _ => panic!("unexpected job"),
    }
}

#[test]
fn pair_plan_single_thread_maps_to_thread_one() {
    let targets = temp_file("pair_targets.txt", "1@a");
    let players = temp_file("pair_players.txt", "3@c");
    let mates = temp_file("pair_mates.txt", "5@e");
    let job = plan_of(&[
        "pair",
        "-l",
        targets.to_str().unwrap(),
        "-p",
        players.to_str().unwrap(),
        "--teammates",
        mates.to_str().unwrap(),
        "-s",
    ])
    .unwrap();
    match job {
        Job::Pair(plan) => assert_eq!(plan.threads, Some(1)),
        _ => panic!("unexpected job"),
    }
}

#[test]
fn cqd_target_preset_and_manual_target_conflict() {
    let err = Cli::try_parse_from(["openbox-cli", "cqd", "-l", "t.txt", "--target-preset", "1", "-p", "p.txt"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

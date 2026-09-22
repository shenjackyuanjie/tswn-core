use std::path::Path;

use super::*;

#[test]
fn to_diy_command_accepts_raw_out_file_and_old_flag() {
    let cli = Cli::try_parse_from(["tswn-cli", "to-diy", "-r", "mario@team", "-o", "out.txt", "--old"]).unwrap();
    match cli.command {
        CliCommand::ToDiy(cmd) => {
            assert_eq!(cmd.raw.as_deref(), Some("mario@team"));
            assert_eq!(cmd.file, None);
            assert_eq!(cmd.out_file.as_deref(), Some(Path::new("out.txt")));
            assert!(cmd.old);
            assert!(!cmd.minions);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn to_diy_command_accepts_minions_flag() {
    let cli = Cli::try_parse_from(["tswn-cli", "to-diy", "-r", "mario@team+shadow", "--minions"]).unwrap();
    match cli.command {
        CliCommand::ToDiy(cmd) => {
            assert_eq!(cmd.raw.as_deref(), Some("mario@team+shadow"));
            assert!(cmd.minions);
            assert!(!cmd.old);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn to_diy_command_rejects_old_with_minions() {
    let err = Cli::try_parse_from(["tswn-cli", "to-diy", "-r", "mario", "--old", "--minions"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn runtime_normalized_run_accepts_raw_and_max_rounds() {
    let cli = Cli::try_parse_from([
        "tswn-cli",
        "runtime",
        "normalized-run",
        "-r",
        "left\\n\\nright",
        "--max-rounds",
        "8",
    ])
    .unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::RuntimeNormalizedRun { raw, max_rounds } => {
            assert_eq!(raw, "left\n\nright");
            assert_eq!(max_rounds, 8);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn diff_uses_main_runtime_only() {
    assert!(Cli::try_parse_from(["tswn-cli", "diff", "-r", "a\\n\\nb"]).is_err());
    let cli = Cli::try_parse_from(["tswn-cli", "runtime", "diff", "-r", "left\\n\\nright"]).unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::RuntimeDiff { raw } => {
            assert_eq!(raw, "left\n\nright");
        }
        _ => panic!("unexpected command"),
    }
    assert!(Cli::try_parse_from(["tswn-cli", "runtime", "diff", "-r", "left\\n\\nright", "--runtime", "legacy"]).is_err());
}

#[test]
fn fight_uses_main_runtime_only() {
    let cli = Cli::try_parse_from(["tswn-cli", "fight", "-r", "left\\n\\nright"]).unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::Fight { raw, jsonl, max_rounds } => {
            assert_eq!(raw, "left\n\nright");
            assert!(!jsonl);
            assert_eq!(max_rounds, 20_000);
        }
        _ => panic!("unexpected command"),
    }
    assert!(Cli::try_parse_from(["tswn-cli", "fight", "-r", "left\\n\\nright", "--runtime", "legacy"]).is_err());
}

#[test]
fn legacy_raw_entry_points_are_rejected() {
    assert!(Cli::try_parse_from(["tswn-cli", "raw", "-r", "left\\n\\nright"]).is_err());
    assert!(Cli::try_parse_from(["tswn-cli", "fight", "-r", "left\\n\\nright", "--out-raw"]).is_err());
}

#[test]
fn namer_pf_accepts_metric_specs() {
    let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario", "--metric", "pp:8000", "--metric", "sum"]).unwrap();
    match cli.command {
        CliCommand::NamerPf(cmd) => {
            assert_eq!(cmd.metrics.len(), 2);
            assert_eq!(cmd.metrics[0].metric, NamerPfMetric::Pp);
            assert_eq!(cmd.metrics[0].min_screen, Some(8000.0));
            assert_eq!(cmd.metrics[1].metric, NamerPfMetric::Sum);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn namer_pf_rejects_legacy_mode_flag() {
    let err = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario", "--mode", "pp"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
}

#[test]
fn namer_pf_rejects_unknown_metric_name() {
    let err = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario", "--metric", "xp"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
}

#[test]
fn namer_pf_defaults_to_all_five_metrics_in_fixed_order() {
    let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario"]).unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::NamerPf {
            metrics,
            no_screen,
            skill_board_config,
            skill_board_output,
            ..
        } => {
            let labels = metrics.iter().map(|spec| spec.metric.label()).collect::<Vec<_>>();
            assert_eq!(labels, vec!["pp", "pd", "qp", "qd", "sum"]);
            assert!(metrics.iter().all(|spec| spec.output_file.is_none() && spec.min_screen.is_none()));
            assert!(!no_screen);
            assert!(skill_board_config.is_none());
            assert!(skill_board_output.is_none());
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn namer_pf_metric_flag_order_does_not_change_output_order() {
    let cli = Cli::try_parse_from([
        "tswn-cli",
        "namer-pf",
        "-r",
        "mario",
        "--metric",
        "sum:30000",
        "--metric",
        "pp:8000",
    ])
    .unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::NamerPf { metrics, .. } => {
            let labels = metrics.iter().map(|spec| spec.metric.label()).collect::<Vec<_>>();
            assert_eq!(labels, vec!["pp", "sum"]);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn namer_pf_rejects_duplicate_metric() {
    let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario", "--metric", "pp", "--metric", "pp:8000"]);
    let err = ParsedCli::from_cli(cli.unwrap()).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
}

#[test]
fn namer_pf_rejects_shared_output_file() {
    let cli = Cli::try_parse_from([
        "tswn-cli",
        "namer-pf",
        "-r",
        "mario",
        "--metric",
        "pp:8000:same.txt",
        "--metric",
        "sum::same.txt",
    ]);
    let err = ParsedCli::from_cli(cli.unwrap()).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
}

#[test]
fn namer_pf_skill_board_out_requires_skill_board() {
    let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario", "--skill-board-out", "board.txt"]);
    let err = ParsedCli::from_cli(cli.unwrap()).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
}

#[test]
fn namer_pf_no_screen_requires_metric_file() {
    let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario", "--no-screen"]);
    let err = ParsedCli::from_cli(cli.unwrap()).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
}

#[test]
fn namer_pf_accepts_keep_rq() {
    let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario", "--keep-rq"]).unwrap();
    match cli.command {
        CliCommand::NamerPf(cmd) => {
            assert!(cmd.keep_rq);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn namer_pf_accepts_precision() {
    let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario", "--precision", "2"]).unwrap();
    match cli.command {
        CliCommand::NamerPf(cmd) => {
            assert_eq!(cmd.precision, 2);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn namer_pf_default_precision_is_zero() {
    let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario"]).unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::NamerPf { precision, .. } => {
            assert_eq!(precision, 0);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn to_diy_command_accepts_no_details() {
    let cli = Cli::try_parse_from(["tswn-cli", "to-diy", "-r", "mario@team", "--no-details"]).unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::ToDiy { details, .. } => assert!(!details),
        _ => panic!("unexpected command"),
    }
}

#[test]
fn to_diy_command_details_default_on() {
    let cli = Cli::try_parse_from(["tswn-cli", "to-diy", "-r", "mario@team"]).unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::ToDiy { details, .. } => assert!(details),
        _ => panic!("unexpected command"),
    }
}

#[test]
fn batch_rate_accepts_new_output_flags() {
    let cli = Cli::try_parse_from([
        "tswn-cli",
        "bench",
        "batch-rate",
        "-l",
        "targets.txt",
        "-p",
        "players.txt",
        "--target-list-double-plus",
        "--show-matchups",
        "--sort",
        "--clean-label",
    ])
    .unwrap();
    match cli.command {
        CliCommand::Bench(BenchCommand {
            command: BenchSubcommand::BatchRate(cmd),
        }) => {
            assert!(cmd.target_list_double_plus);
            assert!(cmd.show_matchups);
            assert!(cmd.sort);
            assert!(cmd.clean_label);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn pair_accepts_teammate_factored_and_detail_flags() {
    let cli = Cli::try_parse_from([
        "tswn-cli",
        "bench",
        "pair",
        "-l",
        "targets.toml",
        "-p",
        "players.txt",
        "--teammate-list",
        "teammates.toml",
        "--head",
        "3",
        "--target-factored",
        "--teammate-factored",
        "--detail",
        "every",
        "--detail-min",
        "60",
        "--sort",
        "--clean-label",
    ])
    .unwrap();
    match cli.command {
        CliCommand::Bench(BenchCommand {
            command: BenchSubcommand::Pair(cmd),
        }) => {
            assert!(cmd.target_factored);
            assert!(cmd.teammate_factored);
            assert_eq!(cmd.detail, PairDetailArg::Every);
            assert_eq!(cmd.detail_min, Some(60.0));
            assert!(cmd.sort);
            assert!(cmd.clean_label);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn pair_detail_defaults_to_none() {
    let cli = Cli::try_parse_from([
        "tswn-cli",
        "bench",
        "pair",
        "-l",
        "targets.txt",
        "-p",
        "players.txt",
        "--teammate-list",
        "teammates.txt",
        "--head",
        "3",
    ])
    .unwrap();
    match cli.command {
        CliCommand::Bench(BenchCommand {
            command: BenchSubcommand::Pair(cmd),
        }) => {
            assert_eq!(cmd.detail, PairDetailArg::None);
            assert_eq!(cmd.detail_min, None);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn bench_win_rate_accepts_raw_two_line_plus_format() {
    let cli = Cli::try_parse_from(["tswn-cli", "bench", "win-rate", "-r", "1@a+2@a\\n3@b+4@b"]).unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::BenchWinRate { team1, team2, .. } => {
            assert_eq!(team1, "1@a\n2@a");
            assert_eq!(team2, "3@b\n4@b");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn bench_win_rate_accepts_double_plus_format() {
    let cli = Cli::try_parse_from([
        "tswn-cli",
        "bench",
        "win-rate",
        "-r",
        "1@a+diy[x]++2@a\\n3@b++4@b",
        "--double-plus",
    ])
    .unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::BenchWinRate { team1, team2, .. } => {
            assert_eq!(team1, "1@a+diy[x]\n2@a");
            assert_eq!(team2, "3@b\n4@b");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn bench_win_rate_rejects_positional_teams() {
    let err = Cli::try_parse_from(["tswn-cli", "bench", "win-rate", "mario", "luigi"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
}

#[test]
fn bench_win_rate_rejects_missing_input() {
    let err = Cli::try_parse_from(["tswn-cli", "bench", "win-rate"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

#[test]
fn batch_rate_rejects_log_and_pure_together() {
    let err = Cli::try_parse_from([
        "tswn-cli",
        "bench",
        "batch-rate",
        "-l",
        "targets.txt",
        "-p",
        "players.txt",
        "-o",
        "out.txt",
        "--log",
        "--pure",
    ])
    .unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn batch_rate_accepts_min_screen_and_min_file() {
    let cli = Cli::try_parse_from([
        "tswn-cli",
        "bench",
        "batch-rate",
        "-l",
        "targets.txt",
        "-p",
        "players.txt",
        "--min-screen",
        "66.5",
        "-o",
        "out.txt",
        "--min-file",
        "70",
    ])
    .unwrap();
    match cli.command {
        CliCommand::Bench(BenchCommand {
            command: BenchSubcommand::BatchRate(cmd),
        }) => {
            assert_eq!(cmd.min_screen, Some(66.5));
            assert_eq!(cmd.min_file, Some(70.0));
            assert_eq!(cmd.wr_precision, 3);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn batch_rate_accepts_wr_precision() {
    let cli = Cli::try_parse_from([
        "tswn-cli",
        "bench",
        "batch-rate",
        "-l",
        "targets.txt",
        "-p",
        "players.txt",
        "--wr-precision",
        "5",
    ])
    .unwrap();
    match cli.command {
        CliCommand::Bench(BenchCommand {
            command: BenchSubcommand::BatchRate(cmd),
        }) => assert_eq!(cmd.wr_precision, 5),
        _ => panic!("unexpected command"),
    }
}

#[test]
fn pair_accepts_required_args_and_wr_precision() {
    let cli = Cli::try_parse_from([
        "tswn-cli",
        "bench",
        "pair",
        "-l",
        "targets.txt",
        "-p",
        "players.txt",
        "--teammate-list",
        "teammates.txt",
        "--head",
        "3",
        "--wr-precision",
        "4",
    ])
    .unwrap();
    match cli.command {
        CliCommand::Bench(BenchCommand {
            command: BenchSubcommand::Pair(cmd),
        }) => {
            assert_eq!(cmd.head, 3);
            assert_eq!(cmd.wr_precision, 4);
            assert_eq!(cmd.teammate_list, PathBuf::from("teammates.txt"));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn pair_accepts_grouping_and_factored_target_flags() {
    let cli = Cli::try_parse_from([
        "tswn-cli",
        "bench",
        "pair",
        "-l",
        "targets.toml",
        "-p",
        "players.txt",
        "--teammate-list",
        "teammates.txt",
        "--head",
        "3",
        "--target-factored",
        "--player-list-double-plus",
        "--teammate-list-single-plus",
    ])
    .unwrap();
    match cli.command {
        CliCommand::Bench(BenchCommand {
            command: BenchSubcommand::Pair(cmd),
        }) => {
            assert!(cmd.target_factored);
            assert!(cmd.player_list_double_plus);
            assert!(cmd.teammate_list_single_plus);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn fight_jsonl_accepts_positive_runtime_round_budget() {
    let cli = Cli::try_parse_from(["tswn-cli", "fight", "-r", "a\\n\\nb", "--jsonl", "--max-rounds", "3"]).unwrap();
    assert!(matches!(
        ParsedCli::from_cli(cli).unwrap().command,
        ParsedCommand::Fight {
            jsonl: true,
            max_rounds: 3,
            ..
        }
    ));
    assert!(Cli::try_parse_from(["tswn-cli", "fight", "-r", "a", "--max-rounds", "0"]).is_err());
}

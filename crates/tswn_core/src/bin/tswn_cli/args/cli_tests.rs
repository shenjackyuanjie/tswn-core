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
fn runtime_parity_accepts_raw_and_max_rounds() {
    let cli = Cli::try_parse_from(["tswn-cli", "runtime", "parity", "-r", "left\\n\\nright", "--max-rounds", "8"]).unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::RuntimeParity { raw, max_rounds } => {
            assert_eq!(raw, "left\n\nright");
            assert_eq!(max_rounds, 8);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn diff_defaults_to_runtime_and_accepts_legacy_runtime() {
    let runtime = Cli::try_parse_from(["tswn-cli", "diff", "-r", "left\\n\\nright"]).unwrap();
    let parsed = ParsedCli::from_cli(runtime).unwrap();
    match parsed.command {
        ParsedCommand::FightDiff { raw, runtime } => {
            assert_eq!(raw, "left\n\nright");
            assert_eq!(runtime, RuntimeEngine::Main);
        }
        _ => panic!("unexpected command"),
    }

    let legacy = Cli::try_parse_from(["tswn-cli", "diff", "-r", "left\\n\\nright", "--runtime", "legacy"]).unwrap();
    let parsed = ParsedCli::from_cli(legacy).unwrap();
    match parsed.command {
        ParsedCommand::FightDiff { raw, runtime } => {
            assert_eq!(raw, "left\n\nright");
            assert_eq!(runtime, RuntimeEngine::Legacy);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn fight_defaults_to_runtime_and_accepts_legacy_runtime() {
    let runtime = Cli::try_parse_from(["tswn-cli", "fight", "-r", "left\\n\\nright"]).unwrap();
    let parsed = ParsedCli::from_cli(runtime).unwrap();
    match parsed.command {
        ParsedCommand::Fight { raw, out_raw, runtime } => {
            assert_eq!(raw, "left\n\nright");
            assert!(!out_raw);
            assert_eq!(runtime, RuntimeEngine::Main);
        }
        _ => panic!("unexpected command"),
    }

    let legacy = Cli::try_parse_from(["tswn-cli", "fight", "-r", "left\\n\\nright", "--out-raw", "--runtime", "legacy"]).unwrap();
    let parsed = ParsedCli::from_cli(legacy).unwrap();
    match parsed.command {
        ParsedCommand::Fight { raw, out_raw, runtime } => {
            assert_eq!(raw, "left\n\nright");
            assert!(out_raw);
            assert_eq!(runtime, RuntimeEngine::Legacy);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn raw_fight_defaults_to_runtime_and_accepts_legacy_runtime() {
    let runtime = Cli::try_parse_from(["tswn-cli", "raw", "-r", "left\\n\\nright"]).unwrap();
    let parsed = ParsedCli::from_cli(runtime).unwrap();
    match parsed.command {
        ParsedCommand::FightRaw {
            raw,
            n,
            threads,
            runtime,
        } => {
            assert_eq!(raw, "left\n\nright");
            assert_eq!(n, 10_000);
            assert_eq!(threads, None);
            assert_eq!(runtime, RuntimeEngine::Main);
        }
        _ => panic!("unexpected command"),
    }

    let legacy = Cli::try_parse_from([
        "tswn-cli",
        "raw",
        "-r",
        "left\\n\\nright",
        "-n",
        "3",
        "-t",
        "2",
        "--runtime",
        "legacy",
    ])
    .unwrap();
    let parsed = ParsedCli::from_cli(legacy).unwrap();
    match parsed.command {
        ParsedCommand::FightRaw {
            raw,
            n,
            threads,
            runtime,
        } => {
            assert_eq!(raw, "left\n\nright");
            assert_eq!(n, 3);
            assert_eq!(threads, Some(2));
            assert_eq!(runtime, RuntimeEngine::Legacy);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn namer_pf_accepts_multiple_modes() {
    let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario", "--mode", "pp", "qd"]).unwrap();
    match cli.command {
        CliCommand::NamerPf(cmd) => {
            assert_eq!(cmd.mode, vec![NamerPfModeArg::Pp, NamerPfModeArg::Qd]);
        }
        _ => panic!("unexpected command"),
    }
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
fn namer_pf_defaults_to_all_modes() {
    let cli = Cli::try_parse_from(["tswn-cli", "namer-pf", "-r", "mario"]).unwrap();
    let parsed = ParsedCli::from_cli(cli).unwrap();
    match parsed.command {
        ParsedCommand::NamerPf { modes, .. } => {
            assert_eq!(modes, NamerPfMode::ALL.to_vec());
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

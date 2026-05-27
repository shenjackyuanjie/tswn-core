//! 命令行参数定义。
//!
//! 使用 `clap` 定义 [`Cli`] 结构体及 [`Command`] 枚举（Run/ShowConfig/Merge/Dedup/Sort/Score/Pair），
//! 以及各子命令所需的选项结构体（[`ScoreCliOptions`]、[`PairCliOptions`]）。

use std::path::PathBuf;

use crate::config::SingleMode;
use crate::error::{Ds3Error, Ds3Result};
use crate::ops::sort::SortOptions;

#[derive(Debug, Clone)]
pub struct Cli {
    pub command: Command,
}

#[derive(Debug, Clone)]
pub enum Command {
    Help,
    Run {
        root: PathBuf,
        config_path: Option<PathBuf>,
    },
    ShowConfig {
        root: PathBuf,
        config_path: Option<PathBuf>,
    },
    Merge {
        input_dir: PathBuf,
        output_file: PathBuf,
    },
    Dedup {
        new_file: PathBuf,
        old_file: PathBuf,
        output_file: PathBuf,
    },
    Sort(SortCli),
    Score(ScoreCli),
    Pair(PairCli),
}

#[derive(Debug, Clone)]
pub struct SortCli {
    pub input_file: PathBuf,
    pub output_file: PathBuf,
    pub score_number: usize,
    pub sort_key_one_based: usize,
    pub output_score: bool,
}

#[derive(Debug, Clone)]
pub struct ScoreCli {
    pub mode: SingleMode,
    pub input_file: PathBuf,
    pub output_file: PathBuf,
    pub score_sieve: i32,
    pub potential_sieve: i32,
    pub threads: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PairCliMode {
    Fc,
    Wc,
    Rh,
}

#[derive(Debug, Clone)]
pub struct PairCli {
    pub mode: PairCliMode,
    pub type_same_set: bool,
    pub left_file: PathBuf,
    pub right_file: PathBuf,
    pub output_file: PathBuf,
    pub sieve: i32,
    pub threads: usize,
}

impl SortCli {
    pub fn to_sort_options(&self) -> SortOptions {
        SortOptions {
            score_number: self.score_number,
            sort_key_zero_based: self.sort_key_one_based.saturating_sub(1),
            output_score: self.output_score,
        }
    }
}

pub fn parse_args<I>(args: I) -> Ds3Result<Cli>
where
    I: IntoIterator<Item = String>,
{
    let mut values = args.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        return Ok(Cli {
            command: Command::Run {
                root: std::env::current_dir()?,
                config_path: None,
            },
        });
    }

    let cmd = values.remove(0);
    match cmd.as_str() {
        "--help" | "-h" | "help" => Ok(Cli { command: Command::Help }),
        "run" => parse_run(values),
        "show-config" => parse_show_config(values),
        "merge" => parse_merge(values),
        "dedup" => parse_dedup(values),
        "sort" => parse_sort(values),
        "score-bc" => parse_score(values, SingleMode::Bc),
        "score-fz" => parse_score(values, SingleMode::Fz),
        "score-wc" => parse_score(values, SingleMode::Wc),
        "score-fs" => parse_score(values, SingleMode::Fs),
        "score-pj" => parse_score(values, SingleMode::Pj),
        "pair-fc" => parse_pair(values, PairCliMode::Fc),
        "pair-wc" => parse_pair(values, PairCliMode::Wc),
        "pair-rh" => parse_pair(values, PairCliMode::Rh),
        _ => Err(Ds3Error::cli(format!("unknown command: {cmd}"))),
    }
}

fn parse_run(args: Vec<String>) -> Ds3Result<Cli> {
    let mut root = std::env::current_dir()?;
    let mut config_path = None;

    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                let value = args.get(idx + 1).ok_or_else(|| Ds3Error::cli("missing value after --root"))?;
                root = PathBuf::from(value);
                idx += 2;
            }
            "--config" => {
                let value = args.get(idx + 1).ok_or_else(|| Ds3Error::cli("missing value after --config"))?;
                config_path = Some(PathBuf::from(value));
                idx += 2;
            }
            unknown => return Err(Ds3Error::cli(format!("unknown run arg: {unknown}"))),
        }
    }

    Ok(Cli {
        command: Command::Run { root, config_path },
    })
}

fn parse_show_config(args: Vec<String>) -> Ds3Result<Cli> {
    let mut root = std::env::current_dir()?;
    let mut config_path = None;

    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                let value = args.get(idx + 1).ok_or_else(|| Ds3Error::cli("missing value after --root"))?;
                root = PathBuf::from(value);
                idx += 2;
            }
            "--config" => {
                let value = args.get(idx + 1).ok_or_else(|| Ds3Error::cli("missing value after --config"))?;
                config_path = Some(PathBuf::from(value));
                idx += 2;
            }
            unknown => return Err(Ds3Error::cli(format!("unknown show-config arg: {unknown}"))),
        }
    }

    Ok(Cli {
        command: Command::ShowConfig { root, config_path },
    })
}

fn parse_merge(args: Vec<String>) -> Ds3Result<Cli> {
    if args.len() != 2 {
        return Err(Ds3Error::cli("merge usage: tswn_ds3 merge <input_dir> <output_file>"));
    }
    Ok(Cli {
        command: Command::Merge {
            input_dir: PathBuf::from(&args[0]),
            output_file: PathBuf::from(&args[1]),
        },
    })
}

fn parse_dedup(args: Vec<String>) -> Ds3Result<Cli> {
    if args.len() != 3 {
        return Err(Ds3Error::cli("dedup usage: tswn_ds3 dedup <new_file> <old_file> <output_file>"));
    }
    Ok(Cli {
        command: Command::Dedup {
            new_file: PathBuf::from(&args[0]),
            old_file: PathBuf::from(&args[1]),
            output_file: PathBuf::from(&args[2]),
        },
    })
}

fn parse_sort(args: Vec<String>) -> Ds3Result<Cli> {
    if args.len() != 5 {
        return Err(Ds3Error::cli(
            "sort usage: tswn_ds3 sort <output_score_0_or_1> <input_file> <output_file> <score_number> <sort_key_1_based>",
        ));
    }
    let output_score = match args[0].as_str() {
        "0" => false,
        "1" => true,
        other => return Err(Ds3Error::cli(format!("output_score must be 0 or 1, got {other}"))),
    };
    let score_number = args[3]
        .parse::<usize>()
        .map_err(|_| Ds3Error::cli(format!("invalid score_number: {}", args[3])))?;
    let sort_key_one_based = args[4]
        .parse::<usize>()
        .map_err(|_| Ds3Error::cli(format!("invalid sort_key: {}", args[4])))?;
    if score_number == 0 {
        return Err(Ds3Error::cli("score_number must be >= 1"));
    }
    if sort_key_one_based == 0 || sort_key_one_based > score_number {
        return Err(Ds3Error::cli(format!(
            "sort_key must be in [1, score_number], got {sort_key_one_based}"
        )));
    }

    Ok(Cli {
        command: Command::Sort(SortCli {
            input_file: PathBuf::from(&args[1]),
            output_file: PathBuf::from(&args[2]),
            score_number,
            sort_key_one_based,
            output_score,
        }),
    })
}

fn parse_score(args: Vec<String>, mode: SingleMode) -> Ds3Result<Cli> {
    if args.len() != 5 {
        return Err(Ds3Error::cli(
            "score usage: tswn_ds3 score-<bc|fz|wc|fs|pj> <threads> <input_file> <output_file> <score_sieve> <potential_sieve>",
        ));
    }
    let threads = args[0]
        .parse::<usize>()
        .map_err(|_| Ds3Error::cli(format!("invalid threads: {}", args[0])))?;
    let score_sieve = args[3]
        .parse::<i32>()
        .map_err(|_| Ds3Error::cli(format!("invalid score_sieve: {}", args[3])))?;
    let potential_sieve = args[4]
        .parse::<i32>()
        .map_err(|_| Ds3Error::cli(format!("invalid potential_sieve: {}", args[4])))?;
    Ok(Cli {
        command: Command::Score(ScoreCli {
            mode,
            input_file: PathBuf::from(&args[1]),
            output_file: PathBuf::from(&args[2]),
            score_sieve,
            potential_sieve,
            threads: threads.max(1),
        }),
    })
}

fn parse_pair(args: Vec<String>, mode: PairCliMode) -> Ds3Result<Cli> {
    if args.len() != 6 {
        return Err(Ds3Error::cli(
            "pair usage: tswn_ds3 pair-<fc|wc|rh> <type_0_or_1> <threads> <sieve> <left_file> <right_file> <output_file>",
        ));
    }
    let type_same_set = match args[0].as_str() {
        "0" => true,
        "1" => false,
        other => return Err(Ds3Error::cli(format!("invalid pair type: {other}, must be 0 or 1"))),
    };
    let threads = args[1]
        .parse::<usize>()
        .map_err(|_| Ds3Error::cli(format!("invalid threads: {}", args[1])))?;
    let sieve = args[2].parse::<i32>().map_err(|_| Ds3Error::cli(format!("invalid sieve: {}", args[2])))?;
    Ok(Cli {
        command: Command::Pair(PairCli {
            mode,
            type_same_set,
            threads: threads.max(1),
            sieve,
            left_file: PathBuf::from(&args[3]),
            right_file: PathBuf::from(&args[4]),
            output_file: PathBuf::from(&args[5]),
        }),
    })
}

pub fn print_usage() {
    println!(
        r#"tswn_ds3 - DS3 rewrite CLI

USAGE:
  tswn_ds3 [command]

COMMANDS:
  run [--root <dir>] [--config <path>]               Run full DS3 pipeline
  show-config [--root <dir>] [--config <path>]       Load and print parsed config
  merge <input_dir> <output_file>                    Merge all input files into one output
  dedup <new_file> <old_file> <output_file>          Remove lines existing in old_file
  sort <0|1> <in> <out> <score_num> <sort_key>       C++ sort.exe compatible mode
  score-bc <t> <in> <out> <s1> <s2>                  Run BC single scoring only
  score-fz <t> <in> <out> <s1> <s2>                  Run FZ single scoring only
  score-wc <t> <in> <out> <s1> <s2>                  Run WC single scoring only
  score-fs <t> <in> <out> <s1> <s2>                  Run FS single scoring only
  score-pj <t> <in> <out> <s1> <s2>                  Run PJ single scoring only
  pair-fc <type> <t> <sieve> <left> <right> <out>    Run FC pairing only
  pair-wc <type> <t> <sieve> <left> <right> <out>    Run WC pairing only
  pair-rh <type> <t> <sieve> <left> <right> <out>    Run RH pairing only
  help                                                Show this help

NOTES:
  - If no command is provided, `run` is used by default.
  - Config lookup order in root: config.toml -> config.txt
"#
    );
}

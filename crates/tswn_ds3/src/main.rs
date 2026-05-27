//! tswn-ds3 — 基于机器学习的命名评分与配对 CLI 工具。
//!
//! 支持五种单字评分模式（BC/FZ/WC/FS/PJ）和三种字对评分模式（FC/RH/WC），
//! 提供输入合并、去重、排序、评分、配对等全流程管道操作。

mod cli;
mod compat;
mod config;
mod error;
mod input;
mod model;
mod ops;
mod output;
mod pairing;
mod pipeline;
mod store;

use std::path::Path;
use std::path::PathBuf;

use cli::Command;
use config::Config;
use error::Ds3Result;
use model::{bc, fs as model_fs, fz, pj, wc as model_wc};
use ops::dedup::remove_duplicates;
use ops::merge::merge_input_files;
use ops::sort::sort_scored_file;
use pairing::{fc, rh, wc as pair_wc};

fn load_config(root: &Path, config_path: Option<PathBuf>) -> Ds3Result<Config> {
    match config_path {
        Some(path) => Config::load_from_path(&path),
        None => Config::load_from_root(root),
    }
}

fn real_main() -> Ds3Result<()> {
    let cli = cli::parse_args(std::env::args().skip(1))?;
    match cli.command {
        Command::Help => {
            cli::print_usage();
        }
        Command::Run { root, config_path } => {
            let config = load_config(&root, config_path)?;
            let report = pipeline::run_full(&root, &config)?;
            println!(
                "run complete: merged_files={}, remaining={}, single(bc/fz/wc/fs/pj)=({}/{}/{}/{}/{}), pair(fc/wc/rh)=({}/{}/{})",
                report.stage1.merged_files,
                report.stage1.dedup.remaining,
                report.single.bc,
                report.single.fz,
                report.single.wc,
                report.single.fs,
                report.single.pj,
                report.pair.fc,
                report.pair.wc,
                report.pair.rh
            );
        }
        Command::ShowConfig { root, config_path } => {
            let config = load_config(&root, config_path)?;
            println!("{config:#?}");
        }
        Command::Merge { input_dir, output_file } => {
            let merged_files = merge_input_files(&input_dir, &output_file)?;
            println!("merged {merged_files} files into {}", output_file.display());
        }
        Command::Dedup {
            new_file,
            old_file,
            output_file,
        } => {
            let stats = remove_duplicates(&new_file, &old_file, &output_file)?;
            println!(
                "dedup complete: new_unique={}, removed_by_old={}, remaining={}",
                stats.new_unique, stats.old_hits, stats.remaining
            );
        }
        Command::Sort(options) => {
            let rows = sort_scored_file(&options.input_file, &options.output_file, &options.to_sort_options())?;
            println!("sort complete: wrote {rows} rows to {}", options.output_file.display());
        }
        Command::Score(options) => {
            let rows = match options.mode {
                config::SingleMode::Bc => bc::score_file_bc_with_threads(
                    &options.input_file,
                    &options.output_file,
                    options.score_sieve,
                    options.potential_sieve,
                    options.threads,
                )?,
                config::SingleMode::Fz => fz::score_file_fz_with_threads(
                    &options.input_file,
                    &options.output_file,
                    options.score_sieve,
                    options.potential_sieve,
                    options.threads,
                )?,
                config::SingleMode::Wc => model_wc::score_file_wc_with_threads(
                    &options.input_file,
                    &options.output_file,
                    options.score_sieve,
                    options.potential_sieve,
                    options.threads,
                )?,
                config::SingleMode::Fs => model_fs::score_file_fs_with_threads(
                    &options.input_file,
                    &options.output_file,
                    options.score_sieve,
                    options.potential_sieve,
                    options.threads,
                )?,
                config::SingleMode::Pj => pj::score_file_pj_with_threads(
                    &options.input_file,
                    &options.output_file,
                    options.score_sieve,
                    options.potential_sieve,
                    options.threads,
                )?,
            };
            println!("score complete: wrote {rows} rows to {}", options.output_file.display());
        }
        Command::Pair(options) => {
            let rows = match options.mode {
                cli::PairCliMode::Fc => fc::run_fc_with_threads(
                    options.type_same_set,
                    &options.left_file,
                    &options.right_file,
                    &options.output_file,
                    options.sieve,
                    options.threads,
                )?,
                cli::PairCliMode::Wc => pair_wc::run_wc_with_threads(
                    options.type_same_set,
                    &options.left_file,
                    &options.right_file,
                    &options.output_file,
                    options.sieve,
                    options.threads,
                )?,
                cli::PairCliMode::Rh => rh::run_rh_with_threads(
                    options.type_same_set,
                    &options.left_file,
                    &options.right_file,
                    &options.output_file,
                    options.sieve,
                    options.threads,
                )?,
            };
            println!("pair complete: wrote {rows} rows to {}", options.output_file.display());
        }
    }
    Ok(())
}

fn main() {
    if let Err(err) = real_main() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

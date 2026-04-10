#![allow(dead_code)]

use std::fs;
use std::path::Path;

use crate::config::{Config, PairMode, SingleMode};
use crate::error::Ds3Result;
use crate::model::{bc, fs as model_fs, fz, pj, wc as model_wc};
use crate::ops::dedup::{DedupStats, remove_duplicates};
use crate::ops::merge::merge_input_files;
use crate::ops::sort::{SortOptions, sort_scored_file};
use crate::output::{append_file, write_bytes_atomic};
use crate::pairing::{fc, rh, wc as pair_wc};
use crate::store::Store;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Stage1Report {
    pub merged_files: usize,
    pub dedup: DedupStats,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SingleReport {
    pub bc: usize,
    pub fz: usize,
    pub wc: usize,
    pub fs: usize,
    pub pj: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PairReport {
    pub fc: usize,
    pub wc: usize,
    pub rh: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FullRunReport {
    pub stage1: Stage1Report,
    pub single: SingleReport,
    pub pair: PairReport,
}

pub fn run_full(root: &Path, config: &Config) -> Ds3Result<FullRunReport> {
    let store = Store::new(root.to_path_buf());
    run_full_with_store(&store, config)
}

pub fn run_full_with_store(store: &Store, config: &Config) -> Ds3Result<FullRunReport> {
    let stage1 = run_stage1_with_store(store, config)?;
    let single = run_single_scoring(store, config)?;
    sort_tmp_single(store)?;
    let pair = run_pairing(store, config)?;
    copy_to_file_store(store)?;
    sort_file_store(store)?;
    if config.copy_to_new {
        copy_to_new_store(store)?;
        sort_new_store(store)?;
    }
    Ok(FullRunReport { stage1, single, pair })
}

pub fn run_stage1(root: &Path, config: &Config) -> Ds3Result<Stage1Report> {
    let store = Store::new(root.to_path_buf());
    run_stage1_with_store(&store, config)
}

pub fn run_stage1_with_store(store: &Store, config: &Config) -> Ds3Result<Stage1Report> {
    prepare_dirs(store)?;
    reset_tmp(store)?;
    let merged_files = merge_input_files(&store.input_dir(), &store.tmp_new_file())?;

    let dedup = if config.run_dedup {
        remove_duplicates(&store.tmp_new_file(), &store.file_old(), &store.tmp_new_dup_file())?
    } else {
        let copied = fs::read(store.tmp_new_file()).unwrap_or_default();
        write_bytes_atomic(&store.tmp_new_dup_file(), &copied)?;
        let remaining = copied.split(|ch| *ch == b'\n').filter(|line| !line.is_empty()).count();
        DedupStats {
            new_unique: remaining,
            old_hits: 0,
            remaining,
        }
    };

    Ok(Stage1Report { merged_files, dedup })
}

fn run_single_scoring(store: &Store, config: &Config) -> Ds3Result<SingleReport> {
    let input = store.tmp_new_dup_file();
    let bc_count = bc::score_file_bc_with_threads(
        &input,
        &store.tmp_new_mode_file(SingleMode::Bc),
        config.single_bc.score,
        config.single_bc.potential,
        config.threads,
    )?;
    let fz_count = fz::score_file_fz_with_threads(
        &input,
        &store.tmp_new_mode_file(SingleMode::Fz),
        config.single_fz.score,
        config.single_fz.potential,
        config.threads,
    )?;
    let wc_count = model_wc::score_file_wc_with_threads(
        &input,
        &store.tmp_new_mode_file(SingleMode::Wc),
        config.single_wc.score,
        config.single_wc.potential,
        config.threads,
    )?;
    let fs_count = model_fs::score_file_fs_with_threads(
        &input,
        &store.tmp_new_mode_file(SingleMode::Fs),
        config.single_fs.score,
        config.single_fs.potential,
        config.threads,
    )?;
    let pj_count = pj::score_file_pj_with_threads(
        &input,
        &store.tmp_new_mode_file(SingleMode::Pj),
        config.single_pj.score,
        config.single_pj.potential,
        config.threads,
    )?;

    Ok(SingleReport {
        bc: bc_count,
        fz: fz_count,
        wc: wc_count,
        fs: fs_count,
        pj: pj_count,
    })
}

fn sort_tmp_single(store: &Store) -> Ds3Result<()> {
    let by_score = SortOptions {
        score_number: 2,
        sort_key_zero_based: 0,
        output_score: true,
    };
    for mode in SingleMode::ALL {
        let path = store.tmp_new_mode_file(mode);
        sort_scored_file(&path, &path, &by_score)?;
    }
    Ok(())
}

fn run_pairing(store: &Store, config: &Config) -> Ds3Result<PairReport> {
    let mut fc_count = 0usize;
    if config.pair_fc.enabled {
        let out = store.out_pair_file(PairMode::Fc);
        fc_count += fc::run_fc_with_threads(
            false,
            &store.tmp_new_mode_file(SingleMode::Fz),
            &store.tmp_new_mode_file(SingleMode::Bc),
            &out,
            config.pair_fc.sieve,
            config.threads,
        )?;
        fc_count += fc::run_fc_with_threads(
            false,
            &store.tmp_new_mode_file(SingleMode::Fz),
            &store.file_old_mode_file(SingleMode::Bc),
            &out,
            config.pair_fc.sieve,
            config.threads,
        )?;
        fc_count += fc::run_fc_with_threads(
            false,
            &store.file_old_mode_file(SingleMode::Fz),
            &store.tmp_new_mode_file(SingleMode::Bc),
            &out,
            config.pair_fc.sieve,
            config.threads,
        )?;
    }

    let mut wc_count = 0usize;
    if config.pair_wc.enabled {
        let out = store.out_pair_file(PairMode::Wc);
        wc_count += pair_wc::run_wc_with_threads(
            true,
            &store.tmp_new_mode_file(SingleMode::Wc),
            &store.tmp_blank_file(),
            &out,
            config.pair_wc.sieve,
            config.threads,
        )?;
        wc_count += pair_wc::run_wc_with_threads(
            false,
            &store.tmp_new_mode_file(SingleMode::Wc),
            &store.file_old_mode_file(SingleMode::Wc),
            &out,
            config.pair_wc.sieve,
            config.threads,
        )?;
    }

    let mut rh_count = 0usize;
    if config.pair_rh.enabled {
        let out = store.out_pair_file(PairMode::Rh);
        rh_count += rh::run_rh_with_threads(
            false,
            &store.tmp_new_mode_file(SingleMode::Fs),
            &store.tmp_new_mode_file(SingleMode::Pj),
            &out,
            config.pair_rh.sieve,
            config.threads,
        )?;
        rh_count += rh::run_rh_with_threads(
            false,
            &store.tmp_new_mode_file(SingleMode::Fs),
            &store.file_old_mode_file(SingleMode::Pj),
            &out,
            config.pair_rh.sieve,
            config.threads,
        )?;
        rh_count += rh::run_rh_with_threads(
            false,
            &store.file_old_mode_file(SingleMode::Fs),
            &store.tmp_new_mode_file(SingleMode::Pj),
            &out,
            config.pair_rh.sieve,
            config.threads,
        )?;
    }

    Ok(PairReport {
        fc: fc_count,
        wc: wc_count,
        rh: rh_count,
    })
}

fn copy_to_file_store(store: &Store) -> Ds3Result<()> {
    append_file(&store.tmp_new_dup_file(), &store.file_old())?;
    for mode in SingleMode::ALL {
        append_file(&store.tmp_new_mode_file(mode), &store.file_old_mode_file(mode))?;
    }
    Ok(())
}

fn sort_file_store(store: &Store) -> Ds3Result<()> {
    let by_score = SortOptions {
        score_number: 2,
        sort_key_zero_based: 0,
        output_score: true,
    };
    let by_potential = SortOptions {
        score_number: 2,
        sort_key_zero_based: 1,
        output_score: true,
    };
    for mode in SingleMode::ALL {
        let old_path = store.file_old_mode_file(mode);
        sort_scored_file(&old_path, &old_path, &by_score)?;
        sort_scored_file(&old_path, &store.file_old_mode_ptt_file(mode), &by_potential)?;
    }
    Ok(())
}

fn copy_to_new_store(store: &Store) -> Ds3Result<()> {
    let new_all = store.new_dir().join("new.txt");
    append_file(&store.tmp_new_dup_file(), &new_all)?;
    append_file(&store.tmp_new_dup_file(), &new_all)?;
    for mode in SingleMode::ALL {
        append_file(&store.tmp_new_mode_file(mode), &store.new_mode_file(mode))?;
    }
    Ok(())
}

fn sort_new_store(store: &Store) -> Ds3Result<()> {
    let by_score = SortOptions {
        score_number: 2,
        sort_key_zero_based: 0,
        output_score: true,
    };
    let by_potential = SortOptions {
        score_number: 2,
        sort_key_zero_based: 1,
        output_score: true,
    };
    for mode in SingleMode::ALL {
        let new_path = store.new_mode_file(mode);
        sort_scored_file(&new_path, &new_path, &by_score)?;
        sort_scored_file(&new_path, &store.new_mode_ptt_file(mode), &by_potential)?;
    }
    Ok(())
}

fn prepare_dirs(store: &Store) -> Ds3Result<()> {
    fs::create_dir_all(store.root())?;
    fs::create_dir_all(store.input_dir())?;
    fs::create_dir_all(store.file_dir())?;
    fs::create_dir_all(store.new_dir())?;
    fs::create_dir_all(store.out_dir())?;
    fs::create_dir_all(store.tmp_dir())?;
    Ok(())
}

fn reset_tmp(store: &Store) -> Ds3Result<()> {
    if store.tmp_dir().exists() {
        fs::remove_dir_all(store.tmp_dir())?;
    }
    fs::create_dir_all(store.tmp_dir())?;
    write_bytes_atomic(&store.tmp_blank_file(), b"1@1\r\n")?;
    Ok(())
}

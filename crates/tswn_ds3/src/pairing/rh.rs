use std::path::Path;

use crate::error::Ds3Result;

use super::coeffs::{MODEL_FS, MODEL_PJ};
use super::common::{PairScoreMode, run_pair_mode, run_pair_mode_with_threads};

pub fn run_rh(type_same_set: bool, left_file: &Path, right_file: &Path, output_file: &Path, sieve: i32) -> Ds3Result<usize> {
    run_pair_mode(
        type_same_set,
        left_file,
        right_file,
        output_file,
        sieve,
        PairScoreMode::RhLeftFsRightPj,
        &MODEL_FS,
        &MODEL_PJ,
    )
}

pub fn run_rh_with_threads(
    type_same_set: bool,
    left_file: &Path,
    right_file: &Path,
    output_file: &Path,
    sieve: i32,
    threads: usize,
) -> Ds3Result<usize> {
    run_pair_mode_with_threads(
        type_same_set,
        left_file,
        right_file,
        output_file,
        sieve,
        PairScoreMode::RhLeftFsRightPj,
        &MODEL_FS,
        &MODEL_PJ,
        threads,
    )
}
